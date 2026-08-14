use poise::serenity_prelude as serenity;
use std::env;
use std::time::Duration;

pub fn parse_upptime_status(json: &serde_json::Value) -> bool {
    if let Some(array) = json.as_array() {
        !array.is_empty()
            && array
                .iter()
                .all(|item| item.get("status").and_then(|s| s.as_str()) == Some("up"))
    } else {
        json.get("status").and_then(|s| s.as_str()) == Some("up")
    }
}

pub fn start_uptime_loop(ctx: serenity::Context, http_client: reqwest::Client) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        let upptime_url = env::var("UPPTIME_URL").unwrap_or_default();

        if upptime_url.is_empty() {
            log::info!("UPPTIME_URL not set. Skipping uptime status task...");
            return;
        }

        loop {
            match http_client.get(&upptime_url).send().await {
                Ok(response) => {
                    match response.json::<serde_json::Value>().await {
                        Ok(json) => {
                            let is_up = parse_upptime_status(&json);

                            let status_text = if is_up {
                                "🟢 All systems operational"
                            } else {
                                "🔴 Some systems offline"
                            };

                            log::info!("Setting bot status to: {}", status_text);
                            ctx.set_activity(Some(serenity::ActivityData::custom(status_text)));
                        }
                        Err(e) => {
                            log::error!("Failed to parse Upptime JSON: {}", e);
                        }
                    }
                }
                Err(e) => {
                    log::error!("Failed to fetch upptime status: {}", e);
                }
            }

            interval.tick().await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_upptime_status_array_all_up() {
        let json = json!([
            {"name": "API", "status": "up"},
            {"name": "Website", "status": "up"}
        ]);
        assert!(parse_upptime_status(&json));
    }

    #[test]
    fn test_parse_upptime_status_array_one_down() {
        let json = json!([
            {"name": "API", "status": "up"},
            {"name": "Website", "status": "down"}
        ]);
        assert!(!parse_upptime_status(&json));
    }

    #[test]
    fn test_parse_upptime_status_single_object() {
        let json = json!({"status": "up"});
        assert!(parse_upptime_status(&json));

        let json_down = json!({"status": "down"});
        assert!(!parse_upptime_status(&json_down));
    }
}
