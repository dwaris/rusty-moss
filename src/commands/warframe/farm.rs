use crate::{Context, Error};
use super::api::get_cached_json;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize, Debug)]
struct ApiResponse {
    #[serde(rename = "missionRewards")]
    mission_rewards: HashMap<String, HashMap<String, MissionData>>,
}

#[derive(Deserialize, Debug)]
struct MissionData {
    #[serde(rename = "gameMode")]
    game_mode: String,
    #[serde(rename = "isEvent")]
    is_event: Option<bool>,
    rewards: MissionRewards,
}

#[derive(Deserialize, Debug)]
#[serde(untagged)]
enum MissionRewards {
    Rotations(HashMap<String, Vec<MissionReward>>),
    List(Vec<MissionReward>),
}

#[derive(Deserialize, Debug)]
struct MissionReward {
    #[serde(rename = "itemName")]
    item_name: String,
    rarity: String,
    chance: f64,
}

fn normalize_relic_name(input: &str) -> Option<String> {
    let parts: Vec<&str> = input.split_whitespace().collect();

    let (tier, code) = match parts.as_slice() {
        [tier, code] => (*tier, *code),
        [tier, code, suffix] if suffix.eq_ignore_ascii_case("relic") => (*tier, *code),
        _ => return None,
    };

    let mut tier_chars = tier.chars();
    let normalized_tier = match tier_chars.next() {
        Some(first) => format!(
            "{}{}",
            first.to_ascii_uppercase(),
            tier_chars.as_str().to_ascii_lowercase()
        ),
        None => return None,
    };

    Some(format!("{} {}", normalized_tier, code.to_ascii_uppercase()))
}

fn parse_relic_and_page(input: &str) -> (String, usize) {
    let trimmed = input.trim();
    let parts: Vec<&str> = trimmed.split_whitespace().collect();

    if parts.len() >= 4 {
        let last = parts[parts.len() - 1];
        let prev = parts[parts.len() - 2];

        if prev.eq_ignore_ascii_case("page") {
            if let Ok(page) = last.parse::<usize>() {
                if page > 0 {
                    let relic = parts[..parts.len() - 2].join(" ");
                    return (relic, page);
                }
            }
        }
    }

    (trimmed.to_string(), 1)
}

/// Find the best missions to farm a specific relic
#[poise::command(slash_command, prefix_command, category = "Warframe")]
pub async fn farm(
    ctx: Context<'_>,
    #[description = "Relic to farm (e.g., 'Lith A1' or 'Axi S9')"]
    #[rest]
    relic: String,
) -> Result<(), Error> {
    ctx.defer().await?;

    let (relic_input, requested_page) = parse_relic_and_page(&relic);

    let Some(normalized_relic) = normalize_relic_name(relic_input.as_str()) else {
        ctx.say("❌ Invalid relic format. Use: 'Lith A1', 'Meso B5', 'Neo Z8', or 'Axi S9'")
            .await?;
        return Ok(());
    };

    let relic_name = format!("{} Relic", normalized_relic);
    let search_term = relic_name.to_lowercase();

    let payload = match get_cached_json(&ctx, "https://drops.warframestat.us/data/missionRewards.json").await {
        Ok(payload) => payload,
        Err(_) => {
            ctx.say("Failed to fetch mission data from API").await?;
            return Ok(());
        }
    };

    let api_response: ApiResponse = serde_json::from_value(payload)?;

    // Search for missions that drop the relic
    let mut found_missions: Vec<(String, String, String, String, f64)> = Vec::new();

    for (planet, missions) in api_response.mission_rewards {
        for (mission_name, mission_data) in missions {
            // Skip event missions
            if mission_data.is_event.unwrap_or(false) {
                continue;
            }

            let full_mission_name = format!("{}/{}", planet, mission_name);

            match &mission_data.rewards {
                MissionRewards::Rotations(rewards_by_rotation) => {
                    for (rotation, rewards) in rewards_by_rotation {
                        for reward in rewards {
                            if reward.item_name.to_lowercase().starts_with(&search_term) {
                                found_missions.push((
                                    full_mission_name.clone(),
                                    mission_data.game_mode.clone(),
                                    rotation.clone(),
                                    reward.rarity.clone(),
                                    reward.chance,
                                ));
                            }
                        }
                    }
                }
                MissionRewards::List(rewards) => {
                    for reward in rewards {
                        if reward.item_name.to_lowercase().starts_with(&search_term) {
                            found_missions.push((
                                full_mission_name.clone(),
                                mission_data.game_mode.clone(),
                                "Any".to_string(),
                                reward.rarity.clone(),
                                reward.chance,
                            ));
                        }
                    }
                }
            }
        }
    }

    if found_missions.is_empty() {
        ctx.say(format!(
            "❌ No missions found that drop **{}**\n\nMake sure to use the format: 'Lith A1', 'Meso B5', 'Neo Z8', or 'Axi S9'. If the format is correct, this relic may not be in the current drop table.",
            relic_name
        ))
        .await?;
        return Ok(());
    }

    // Sort by drop chance (highest first)
    found_missions.sort_by(|a, b| b.4.partial_cmp(&a.4).unwrap());

    // Group by mission and show best rotation
    let mut mission_map: HashMap<String, Vec<(String, String, f64)>> = HashMap::new();
    for (mission, game_mode, rotation, rarity, chance) in found_missions {
        mission_map
            .entry(format!("{} [{}]", mission, game_mode))
            .or_insert_with(Vec::new)
            .push((rotation, rarity, chance));
    }

    let mut sorted_missions: Vec<_> = mission_map.into_iter().collect();
    sorted_missions.sort_by(|a, b| {
        let max_a = a.1.iter().map(|x| x.2).fold(0.0, f64::max);
        let max_b = b.1.iter().map(|x| x.2).fold(0.0, f64::max);
        max_b.partial_cmp(&max_a).unwrap()
    });

    let page_size = 10;
    let total_pages = sorted_missions.len().div_ceil(page_size);

    if requested_page > total_pages {
        ctx.say(format!(
            "Page {} does not exist. There are {} pages for {}.",
            requested_page, total_pages, relic_name
        ))
        .await?;
        return Ok(());
    }

    let start = (requested_page - 1) * page_size;
    let end = usize::min(start + page_size, sorted_missions.len());

    // Format response
    let mut response_text = format!(
        "Best missions to farm {} (Page {}/{}):\n\n",
        relic_name, requested_page, total_pages
    );

    for (mission, rotations) in sorted_missions[start..end].iter() {
        response_text.push_str(&format!("{}\n", mission));

        for (rotation, rarity, chance) in rotations {
            response_text.push_str(&format!(
                "  Rotation {} - {}: {:.2}%\n",
                rotation, rarity, chance
            ));
        }
        response_text.push('\n');
    }

    if total_pages > 1 {
        response_text.push_str(&format!(
            "Use: farm {} page <n> to view another page.",
            normalized_relic
        ));
    }

    ctx.say(response_text).await?;
    Ok(())
}
