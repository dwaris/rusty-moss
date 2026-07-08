use poise::serenity_prelude as serenity;
use std::env;
use std::time::Duration;

pub fn start_uptime_loop(ctx: serenity::Context, http_client: reqwest::Client) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        let upptime_url = env::var("UPPTIME_URL").unwrap_or_default();
        
        if upptime_url.is_empty() {
            println!("UPPTIME_URL not set. Skipping ...");
            return;
        }

        loop {
            match http_client.get(&upptime_url).send().await {
                Ok(response) => {
                    match response.json::<serde_json::Value>().await {
                        Ok(json) => {
                            let is_up = if let Some(array) = json.as_array() {
                                array.iter().all(|item| item.get("status").and_then(|s| s.as_str()) == Some("up"))
                            } else {
                                json.get("status").and_then(|s| s.as_str()) == Some("up")
                            };
                            
                            let status_text = if is_up {
                                "🟢 All systems operational"
                            } else {
                                "🔴 Some systems offline"
                            };
                            
                            println!("Setting bot status to: {}", status_text);
                            ctx.set_activity(Some(serenity::ActivityData::custom(status_text)));
                        }
                        Err(e) => {
                            println!("Failed to parse Upptime JSON: {}", e);
                        }
                    }
                }
                Err(e) => {
                    println!("Failed to fetch upptime status: {}", e);
                }
            }
            
            interval.tick().await;
        }
    });
}
