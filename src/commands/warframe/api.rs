use crate::commands::warframe::types::WarframeError;
use crate::{Context, Error};
use serde::de::DeserializeOwned;
use std::sync::Arc;

pub async fn get_cached_json<T: DeserializeOwned + Send + Sync + 'static>(
    ctx: &Context<'_>,
    url: &str,
) -> Result<Arc<T>, Error> {
    if let Some(cached) = ctx.data().warframe_cache.get(url).await
        && let Ok(val) = cached.downcast::<T>()
    {
        return Ok(val);
    }

    let response = ctx.data().http_client.get(url).send().await?;

    if !response.status().is_success() {
        return Err(WarframeError::HttpError {
            url: url.to_string(),
            status: response.status(),
        }
        .into());
    }

    let parsed: T = response.json().await.map_err(|e| {
        log::error!("Failed to deserialize API response from {url}: {e}");
        WarframeError::Deserialize {
            url: url.to_string(),
            source: e,
        }
    })?;
    let data = Arc::new(parsed);

    ctx.data()
        .warframe_cache
        .insert(url.to_string(), data.clone())
        .await;

    Ok(data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::any::Any;
    use std::time::Duration;

    #[tokio::test]
    async fn test_moka_cache_downcast_and_expiry() {
        let cache: moka::future::Cache<String, Arc<dyn Any + Send + Sync>> =
            moka::future::Cache::builder()
                .time_to_live(Duration::from_millis(50))
                .build();

        let value = Arc::new("cached test payload".to_string());
        cache.insert("test_key".to_string(), value).await;

        let retrieved = cache.get("test_key").await.expect("Key should exist");
        let downcasted = retrieved
            .downcast::<String>()
            .expect("Downcast must succeed");
        assert_eq!(&*downcasted, "cached test payload");

        tokio::time::sleep(Duration::from_millis(70)).await;
        assert!(cache.get("test_key").await.is_none());
    }
}
