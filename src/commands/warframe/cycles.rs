use super::api::get_cached_json;
use crate::{Context, Error};
use poise::serenity_prelude as serenity;
use serde::Deserialize;

fn deserialize_optional_datetime<'de, D>(
    deserializer: D,
) -> Result<Option<chrono::DateTime<chrono::Utc>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let opt = Option::<String>::deserialize(deserializer)?;
    match opt.as_deref() {
        None | Some("") => Ok(None),
        Some(s) => chrono::DateTime::parse_from_rfc3339(s)
            .map(|dt| Some(dt.with_timezone(&chrono::Utc)))
            .map_err(serde::de::Error::custom),
    }
}

#[derive(Deserialize, Debug)]
pub struct CycleResponse {
    #[serde(default)]
    state: String,
    #[serde(rename = "isDay")]
    is_day: Option<bool>,
    #[serde(rename = "isWarm")]
    is_warm: Option<bool>,
    #[serde(rename = "timeLeft")]
    time_left: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_datetime")]
    expiry: Option<chrono::DateTime<chrono::Utc>>,
}

fn format_status(
    state_name: &str,
    time_left: Option<&str>,
    expiry: Option<chrono::DateTime<chrono::Utc>>,
) -> String {
    let mut time_str = String::new();

    if let Some(tl) = time_left.filter(|s| !s.is_empty()) {
        time_str = format!(" ({tl} remaining)");
    } else if let Some(target) = expiry {
        let now = chrono::Utc::now();
        let duration = target - now;
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

    let (cetus, vallis, cambion, earth) = tokio::join!(
        get_cached_json::<CycleResponse>(&ctx, "https://api.warframestat.us/pc/cetusCycle"),
        get_cached_json::<CycleResponse>(&ctx, "https://api.warframestat.us/pc/vallisCycle"),
        get_cached_json::<CycleResponse>(&ctx, "https://api.warframestat.us/pc/cambionCycle"),
        get_cached_json::<CycleResponse>(&ctx, "https://api.warframestat.us/pc/earthCycle"),
    );

    let cetus_str = match cetus {
        Ok(c) => {
            let state = if c.is_day.unwrap_or(false) || c.state.eq_ignore_ascii_case("day") {
                "Day"
            } else {
                "Night"
            };
            format_status(state, c.time_left.as_deref(), c.expiry)
        }
        Err(e) => {
            log::warn!("Failed to fetch Cetus cycle: {e}");
            "Unavailable".to_string()
        }
    };

    let vallis_str = match vallis {
        Ok(v) => {
            let state = if v.is_warm.unwrap_or(false) || v.state.eq_ignore_ascii_case("warm") {
                "Warm"
            } else {
                "Cold"
            };
            format_status(state, v.time_left.as_deref(), v.expiry)
        }
        Err(e) => {
            log::warn!("Failed to fetch Vallis cycle: {e}");
            "Unavailable".to_string()
        }
    };

    let cambion_str = match cambion {
        Ok(c) => {
            let state = if c.state.eq_ignore_ascii_case("vome") {
                "Vome"
            } else if c.state.eq_ignore_ascii_case("fass") {
                "Fass"
            } else {
                "Unknown"
            };
            format_status(state, c.time_left.as_deref(), c.expiry)
        }
        Err(e) => {
            log::warn!("Failed to fetch Cambion cycle: {e}");
            "Unavailable".to_string()
        }
    };

    let earth_str = match earth {
        Ok(e) => {
            let state = if e.is_day.unwrap_or(false) || e.state.eq_ignore_ascii_case("day") {
                "Day"
            } else {
                "Night"
            };
            format_status(state, e.time_left.as_deref(), e.expiry)
        }
        Err(e) => {
            log::warn!("Failed to fetch Earth cycle: {e}");
            "Unavailable".to_string()
        }
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
        let formatted = format_status("Day", Some("1h 8m 1s"), None);
        assert_eq!(formatted, "**Day** (1h 8m 1s remaining)");
    }

    #[test]
    fn test_parse_cycle_json() {
        let json_data = r#"{"id":"cetus1","isDay":true,"state":"day","timeLeft":"50m 10s","expiry":"2026-09-06T20:15:00.000Z"}"#;
        let cycle: CycleResponse = serde_json::from_str(json_data).unwrap();
        assert_eq!(cycle.state, "day");
        assert_eq!(cycle.is_day, Some(true));
        assert_eq!(cycle.time_left.as_deref(), Some("50m 10s"));
        assert!(cycle.expiry.is_some());
    }

    #[test]
    fn test_parse_cycle_json_empty_or_null_expiry() {
        let json_empty = r#"{"state":"night","timeLeft":"","expiry":""}"#;
        let cycle_empty: CycleResponse = serde_json::from_str(json_empty).unwrap();
        assert!(cycle_empty.expiry.is_none());

        let json_null = r#"{"state":"night","timeLeft":"","expiry":null}"#;
        let cycle_null: CycleResponse = serde_json::from_str(json_null).unwrap();
        assert!(cycle_null.expiry.is_none());

        let json_omitted = r#"{"state":"night"}"#;
        let cycle_omitted: CycleResponse = serde_json::from_str(json_omitted).unwrap();
        assert!(cycle_omitted.expiry.is_none());
    }
}
