use crate::{Context, Error};
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

pub fn prune_cache(cache: &mut HashMap<String, crate::CachedItem>, ttl: Duration) {
    cache.retain(|_, v| v.fetched_at.elapsed() <= ttl);
}

pub async fn get_cached_json<T: DeserializeOwned + Send + Sync + 'static>(
    ctx: &Context<'_>,
    url: &str,
) -> Result<Arc<T>, Error> {
    {
        let cache = ctx.data().warframe_cache.read().await;
        if let Some(cached) = cache.get(url)
            && cached.fetched_at.elapsed() <= ctx.data().warframe_cache_ttl
            && let Ok(val) = cached.data.clone().downcast::<T>()
        {
            return Ok(val);
        }
    }

    let response = ctx.data().http_client.get(url).send().await?;

    if !response.status().is_success() {
        return Err(format!(
            "Warframe API request to {url} failed with status {}",
            response.status()
        )
        .into());
    }

    let parsed: T = response.json().await.map_err(|e| {
        log::error!("Failed to deserialize API response from {url}: {e}");
        e
    })?;
    let data = Arc::new(parsed);

    {
        let mut cache = ctx.data().warframe_cache.write().await;
        let ttl = ctx.data().warframe_cache_ttl;
        prune_cache(&mut cache, ttl);

        cache.insert(
            url.to_string(),
            crate::CachedItem {
                fetched_at: std::time::Instant::now(),
                data: data.clone(),
            },
        );
    }

    Ok(data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn test_prune_cache_removes_expired_entries() {
        let mut cache = HashMap::new();
        let ttl = Duration::from_secs(10);

        cache.insert(
            "https://api.example.com/fresh".to_string(),
            crate::CachedItem {
                fetched_at: Instant::now(),
                data: Arc::new("fresh".to_string()),
            },
        );

        cache.insert(
            "https://api.example.com/expired".to_string(),
            crate::CachedItem {
                fetched_at: Instant::now() - Duration::from_secs(20),
                data: Arc::new("expired".to_string()),
            },
        );

        prune_cache(&mut cache, ttl);

        assert!(cache.contains_key("https://api.example.com/fresh"));
        assert!(!cache.contains_key("https://api.example.com/expired"));
        assert_eq!(cache.len(), 1);
    }
}
