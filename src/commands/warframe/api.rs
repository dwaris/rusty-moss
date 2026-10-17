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
        tracing::error!("Failed to deserialize API response from {url}: {e}");
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
