use super::api::get_cached_json;
use crate::{Context, Error};
use poise::serenity_prelude as serenity;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
struct CycleResponse {
    #[serde(default)]
    state: String,
    #[serde(rename = "isDay")]
    is_day: Option<bool>,
    #[serde(rename = "isWarm")]
    is_warm: Option<bool>,
    #[serde(rename = "timeLeft")]
    time_left: Option<String>,
    expiry: Option<String>,
}

fn format_status(
    state_name: String,
    time_left: Option<&str>,
    expiry: Option<&str>,
) -> String {
    let mut time_str = String::new();

    if let Some(tl) = time_left.filter(|s| !s.is_empty()) {
        time_str = format!(" ({tl} remaining)");
    } else if let Some(exp) = expiry.filter(|s| !s.is_empty()) {
        if let Ok(target) = chrono::DateTime::parse_from_rfc3339(exp) {
            let now = chrono::Utc::now();
            let duration = target.with_timezone(&chrono::Utc) - now;
            let total_secs = duration.num_seconds();

            if total_secs > 0 {
                let mins = total_secs / 60;
                let secs = total_secs % 60;
                if mins > 0 {
                    time_str = format!(" ({mins}m {secs}s remaining)");
                } else {
                    time_str = format!(" ({secs}s remaining)");
                }
            }
        }
    }

    format!("**{state_name}**{time_str}")
}

/// Check current open-world day/night and temperature cycles in Warframe
#[poise::command(
    slash_command,
    prefix_command,
    aliases("cycles"),
    category = "Warframe"
)]
pub async fn cycle(ctx: Context<'_>) -> Result<(), Error> {
    ctx.defer().await?;

    let cetus: Result<CycleResponse, _> =
        get_cached_json(&ctx, "https://api.warframestat.us/pc/cetusCycle").await;
    let vallis: Result<CycleResponse, _> =
        get_cached_json(&ctx, "https://api.warframestat.us/pc/vallisCycle").await;
    let cambion: Result<CycleResponse, _> =
        get_cached_json(&ctx, "https://api.warframestat.us/pc/cambionCycle").await;
    let earth: Result<CycleResponse, _> =
        get_cached_json(&ctx, "https://api.warframestat.us/pc/earthCycle").await;

    let cetus_str = match cetus {
        Ok(c) => {
            let state = if c.is_day.unwrap_or(false) || c.state.eq_ignore_ascii_case("day") {
                "Day".to_string()
            } else {
                "Night".to_string()
            };
            format_status(state, c.time_left.as_deref(), c.expiry.as_deref())
        }
        Err(_) => "Unavailable".to_string(),
    };

    let vallis_str = match vallis {
        Ok(v) => {
            let state = if v.is_warm.unwrap_or(false) || v.state.eq_ignore_ascii_case("warm") {
                "Warm".to_string()
            } else {
                "Cold".to_string()
            };
            format_status(state, v.time_left.as_deref(), v.expiry.as_deref())
        }
        Err(_) => "Unavailable".to_string(),
    };

    let cambion_str = match cambion {
        Ok(c) => {
            let state = if c.state.eq_ignore_ascii_case("vome") {
                "Vome".to_string()
            } else {
                "Fass".to_string()
            };
            format_status(state, c.time_left.as_deref(), c.expiry.as_deref())
        }
        Err(_) => "Unavailable".to_string(),
    };

    let earth_str = match earth {
        Ok(e) => {
            let state = if e.is_day.unwrap_or(false) || e.state.eq_ignore_ascii_case("day") {
                "Day".to_string()
            } else {
                "Night".to_string()
            };
            format_status(state, e.time_left.as_deref(), e.expiry.as_deref())
        }
        Err(_) => "Unavailable".to_string(),
    };

    let embed = serenity::CreateEmbed::new()
        .title("Warframe Worldstate Cycles")
        .color(0x2f3136)
        .field("Cetus", cetus_str, false)
        .field("Orb Vallis", vallis_str, false)
        .field("Cambion Drift", cambion_str, false)
        .field("Earth", earth_str, false);

    ctx.send(poise::CreateReply::default().embed(embed)).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_status_with_time_left() {
        let formatted = format_status("Day".to_string(), Some("1h 8m 1s"), None);
        assert_eq!(formatted, "**Day** (1h 8m 1s remaining)");
    }

    #[test]
    fn test_format_status_empty() {
        let formatted = format_status("Night".to_string(), None, None);
        assert_eq!(formatted, "**Night**");
    }

    #[test]
    fn test_parse_cycle_json() {
        let json_data = r#"{"id":"cetus1","isDay":true,"state":"day","timeLeft":"50m 10s"}"#;
        let cycle: CycleResponse = serde_json::from_str(json_data).unwrap();
        assert_eq!(cycle.state, "day");
        assert_eq!(cycle.is_day, Some(true));
        assert_eq!(cycle.time_left.as_deref(), Some("50m 10s"));
    }
}
