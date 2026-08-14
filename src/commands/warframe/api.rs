use crate::{Context, Error};
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::time::Duration;

pub fn prune_cache(cache: &mut HashMap<String, crate::CachedPayload>, ttl: Duration) {
    cache.retain(|_, v| v.fetched_at.elapsed() <= ttl);
}

pub async fn get_cached_json<T: DeserializeOwned + 'static>(
    ctx: &Context<'_>,
    url: &str,
) -> Result<T, Error> {
    {
        let cache = ctx.data().warframe_cache.read().await;
        if let Some(cached) = cache.get(url) {
            if cached.fetched_at.elapsed() <= ctx.data().warframe_cache_ttl {
                if let Ok(val) = serde_json::from_value::<T>(cached.payload.clone()) {
                    return Ok(val);
                }
            }
        }
    }

    let response = ctx.data().http_client.get(url).send().await?;

    if !response.status().is_success() {
        return Err(format!(
            "Warframe API request failed with status {}",
            response.status()
        )
        .into());
    }

    let payload: serde_json::Value = response.json().await?;

    {
        let mut cache = ctx.data().warframe_cache.write().await;
        let ttl = ctx.data().warframe_cache_ttl;
        prune_cache(&mut cache, ttl);

        cache.insert(
            url.to_string(),
            crate::CachedPayload {
                fetched_at: std::time::Instant::now(),
                payload: payload.clone(),
            },
        );
    }

    Ok(serde_json::from_value::<T>(payload)?)
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
            crate::CachedPayload {
                fetched_at: Instant::now(),
                payload: serde_json::json!({"status": "fresh"}),
            },
        );

        cache.insert(
            "https://api.example.com/expired".to_string(),
            crate::CachedPayload {
                fetched_at: Instant::now() - Duration::from_secs(20),
                payload: serde_json::json!({"status": "expired"}),
            },
        );

        prune_cache(&mut cache, ttl);

        assert!(cache.contains_key("https://api.example.com/fresh"));
        assert!(!cache.contains_key("https://api.example.com/expired"));
        assert_eq!(cache.len(), 1);
    }
}
