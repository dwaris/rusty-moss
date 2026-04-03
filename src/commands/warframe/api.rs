use crate::{Context, Error};

pub async fn get_cached_json(ctx: &Context<'_>, url: &str) -> Result<serde_json::Value, Error> {
    {
        let cache = ctx.data().warframe_cache.read().await;
        if let Some(cached) = cache.get(url) {
            if cached.fetched_at.elapsed() <= ctx.data().warframe_cache_ttl {
                return Ok(cached.payload.clone());
            }
        }
    }

    let response = ctx.data().http_client.get(url).send().await?;

    if !response.status().is_success() {
        return Err(format!("Warframe API request failed with status {}", response.status()).into());
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

    Ok(payload)
}
