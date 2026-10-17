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
    pub state: String,
    #[serde(rename = "isDay")]
    pub is_day: Option<bool>,
    #[serde(rename = "isWarm")]
    pub is_warm: Option<bool>,
    #[serde(rename = "timeLeft")]
    pub time_left: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_datetime")]
    pub expiry: Option<chrono::DateTime<chrono::Utc>>,
}

pub struct RegionCycleDef {
    pub display_name: &'static str,
    pub endpoint: &'static str,
}

pub const REGIONS: &[RegionCycleDef] = &[
    RegionCycleDef {
        display_name: "Cetus",
        endpoint: "https://api.warframestat.us/pc/cetusCycle",
    },
    RegionCycleDef {
        display_name: "Orb Vallis",
        endpoint: "https://api.warframestat.us/pc/vallisCycle",
    },
    RegionCycleDef {
        display_name: "Cambion Drift",
        endpoint: "https://api.warframestat.us/pc/cambionCycle",
    },
    RegionCycleDef {
        display_name: "Earth",
        endpoint: "https://api.warframestat.us/pc/earthCycle",
    },
    RegionCycleDef {
        display_name: "Zariman",
        endpoint: "https://api.warframestat.us/pc/zarimanCycle",
    },
    RegionCycleDef {
        display_name: "Duviri",
        endpoint: "https://api.warframestat.us/pc/duviriCycle",
    },
];

impl CycleResponse {
    pub fn phase_name(&self) -> String {
        let trimmed = self.state.trim();
        if !trimmed.is_empty() {
            let mut chars = trimmed.chars();
            match chars.next() {
                Some(first) => format!(
                    "{}{}",
                    first.to_ascii_uppercase(),
                    chars.as_str().to_ascii_lowercase()
                ),
                None => "Unknown".to_string(),
            }
        } else if let Some(is_day) = self.is_day {
            if is_day {
                "Day".to_string()
            } else {
                "Night".to_string()
            }
        } else if let Some(is_warm) = self.is_warm {
            if is_warm {
                "Warm".to_string()
            } else {
                "Cold".to_string()
            }
        } else {
            "Unknown".to_string()
        }
    }

    pub fn format_status(&self) -> String {
        let phase = self.phase_name();
        format_cycle_display(&phase, self.time_left.as_deref(), self.expiry)
    }
}

pub fn format_cycle_display(
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

    let futures = REGIONS.iter().map(|region| async move {
        let result = get_cached_json::<CycleResponse>(&ctx, region.endpoint).await;
        let formatted = match result {
            Ok(c) => c.format_status(),
            Err(e) => {
                tracing::warn!("Failed to fetch {} cycle: {e}", region.display_name);
                "Unavailable".to_string()
            }
        };
        (region.display_name, formatted)
    });

    let results = serenity::futures::future::join_all(futures).await;

    let mut embed = serenity::CreateEmbed::new()
        .title("Warframe Worldstate Cycles")
        .color(0x2f3136);

    for (name, status) in results {
        embed = embed.field(name, status, false);
    }

    ctx.send(poise::CreateReply::default().embed(embed)).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_status_with_time_left() {
        let formatted = format_cycle_display("Day", Some("1h 8m 1s"), None);
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

        let json_omitted = r#"{"state":"night"}"#;
        let cycle_omitted: CycleResponse = serde_json::from_str(json_omitted).unwrap();
        assert!(cycle_omitted.expiry.is_none());
    }

    #[test]
    fn test_cycle_phases() {
        let cetus: CycleResponse = serde_json::from_str(r#"{"state":"day"}"#).unwrap();
        assert_eq!(cetus.phase_name(), "Day");

        let vallis: CycleResponse = serde_json::from_str(r#"{"isWarm":true}"#).unwrap();
        assert_eq!(vallis.phase_name(), "Warm");

        let cambion: CycleResponse = serde_json::from_str(r#"{"state":"vome"}"#).unwrap();
        assert_eq!(cambion.phase_name(), "Vome");
        assert_eq!(cambion.format_status(), "**Vome**");

        let zariman: CycleResponse = serde_json::from_str(r#"{"state":"corpus"}"#).unwrap();
        assert_eq!(zariman.phase_name(), "Corpus");

        let duviri: CycleResponse = serde_json::from_str(r#"{"state":"fear"}"#).unwrap();
        assert_eq!(duviri.phase_name(), "Fear");
    }
}
