use crate::{Context, Error};
use serde::de::DeserializeOwned;

pub async fn get_cached_json<T: DeserializeOwned + Clone + 'static>(
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
