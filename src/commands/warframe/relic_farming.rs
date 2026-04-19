use super::api::get_cached_json;
use super::normalization::{normalize_whitespace, starts_with_prefix_ignore_ascii_case};
use crate::{Context, Error};
use poise::serenity_prelude as serenity;
use serde::Deserialize;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::time::Duration;

type RotationDrop = (String, String, f64);
type MissionDrops = (String, Vec<RotationDrop>);
const MISSION_PAGE_SIZE: usize = 10;

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

#[derive(Debug)]
struct FoundDrop {
    mission_path: String,
    game_mode: String,
    rotation: String,
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

fn relic_name_from_item(item_name: &str) -> Option<String> {
    let cleaned = normalize_whitespace(item_name);
    cleaned.strip_suffix(" Relic").map(str::to_string)
}

fn for_each_mission_reward<F>(mission_data: &MissionData, mut callback: F)
where
    F: FnMut(&str, &MissionReward),
{
    match &mission_data.rewards {
        MissionRewards::Rotations(rewards_by_rotation) => {
            for (rotation, rewards) in rewards_by_rotation {
                for reward in rewards {
                    callback(rotation, reward);
                }
            }
        }
        MissionRewards::List(rewards) => {
            for reward in rewards {
                callback("Any", reward);
            }
        }
    }
}

async fn fetch_mission_rewards(ctx: &Context<'_>) -> Result<ApiResponse, Error> {
    get_cached_json(ctx, "https://drops.warframestat.us/data/missionRewards.json").await
}

fn collect_relic_name_suggestions(api_response: &ApiResponse, partial: &str) -> Vec<String> {
    let partial = partial.trim();
    let mut unique = HashSet::new();

    for missions in api_response.mission_rewards.values() {
        for mission_data in missions.values() {
            for_each_mission_reward(mission_data, |_, reward| {
                let Some(name) = relic_name_from_item(&reward.item_name) else {
                    return;
                };

                let Some(normalized) = normalize_relic_name(&name) else {
                    return;
                };

                if starts_with_prefix_ignore_ascii_case(&normalized, partial) {
                    unique.insert(normalized);
                }
            });
        }
    }

    let mut suggestions: Vec<String> = unique.into_iter().collect();
    suggestions.sort();
    suggestions.truncate(25);
    suggestions
}

async fn farm_relic_autocomplete(ctx: Context<'_>, partial: &str) -> Vec<String> {
    let api_response: ApiResponse = match fetch_mission_rewards(&ctx).await {
        Ok(response) => response,
        Err(_) => return Vec::new(),
    };

    collect_relic_name_suggestions(&api_response, partial)
}

fn page_components(current_page: usize, total_pages: usize) -> Vec<serenity::CreateActionRow> {
    let prev_button = serenity::CreateButton::new("farm_prev")
        .label("Previous")
        .style(serenity::ButtonStyle::Primary)
        .disabled(current_page == 0);

    let next_button = serenity::CreateButton::new("farm_next")
        .label("Next")
        .style(serenity::ButtonStyle::Primary)
        .disabled(current_page + 1 >= total_pages);

    vec![serenity::CreateActionRow::Buttons(vec![prev_button, next_button])]
}

fn collect_found_drops(api_response: ApiResponse, search_term: &str) -> Vec<FoundDrop> {
    let mut found = Vec::new();

    for (planet, missions) in api_response.mission_rewards {
        for (mission_name, mission_data) in missions {
            if mission_data.is_event.unwrap_or(false) {
                continue;
            }

            let mission_path = format!("{}/{}", planet, mission_name);

            for_each_mission_reward(&mission_data, |rotation, reward| {
                if !starts_with_prefix_ignore_ascii_case(&reward.item_name, search_term) {
                    return;
                }

                found.push(FoundDrop {
                    mission_path: mission_path.clone(),
                    game_mode: mission_data.game_mode.clone(),
                    rotation: rotation.to_string(),
                    rarity: reward.rarity.clone(),
                    chance: reward.chance,
                });
            });
        }
    }

    found
}

fn group_and_sort_missions(found_drops: Vec<FoundDrop>) -> Vec<MissionDrops> {
    let mut mission_map: HashMap<String, Vec<RotationDrop>> = HashMap::new();

    for drop in found_drops {
        let mission_label = format!("{} [{}]", drop.mission_path, drop.game_mode);
        mission_map
            .entry(mission_label)
            .or_default()
            .push((drop.rotation, drop.rarity, drop.chance));
    }

    for drops in mission_map.values_mut() {
        drops.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(Ordering::Equal));
    }

    let mut missions: Vec<MissionDrops> = mission_map.into_iter().collect();
    missions.sort_by(|a, b| {
        let best_a = a.1.iter().map(|x| x.2).fold(0.0, f64::max);
        let best_b = b.1.iter().map(|x| x.2).fold(0.0, f64::max);
        best_b.partial_cmp(&best_a).unwrap_or(Ordering::Equal)
    });

    missions
}

fn paginate_missions(missions: Vec<MissionDrops>, page_size: usize) -> Vec<Vec<MissionDrops>> {
    missions
        .chunks(page_size)
        .map(|chunk| chunk.to_vec())
        .collect()
}

fn format_page(
    relic_name: &str,
    current_page: usize,
    total_pages: usize,
    missions: &[MissionDrops],
) -> String {
    let mut response_text = format!(
        "Best missions to farm {} (Page {}/{}):\n\n",
        relic_name,
        current_page + 1,
        total_pages
    );

    for (mission, rotations) in missions {
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
        response_text.push_str("Use the buttons below to change pages.");
    }

    response_text
}

async fn handle_page_interactions(
    ctx: Context<'_>,
    mut message: serenity::Message,
    relic_name: &str,
    total_pages: usize,
    pages: &[Vec<MissionDrops>],
    current_page: &mut usize,
) -> Result<(), Error> {
    while let Some(interaction) = serenity::collector::ComponentInteractionCollector::new(ctx.serenity_context())
        .author_id(ctx.author().id)
        .channel_id(ctx.channel_id())
        .message_id(message.id)
        .timeout(Duration::from_secs(120))
        .await
    {
        match interaction.data.custom_id.as_str() {
            "farm_prev" if *current_page > 0 => *current_page -= 1,
            "farm_next" if *current_page + 1 < total_pages => *current_page += 1,
            _ => {}
        }

        interaction
            .create_response(
                ctx.serenity_context(),
                serenity::CreateInteractionResponse::UpdateMessage(
                    serenity::CreateInteractionResponseMessage::new()
                        .content(format_page(
                            relic_name,
                            *current_page,
                            total_pages,
                            &pages[*current_page],
                        ))
                        .components(page_components(*current_page, total_pages)),
                ),
            )
            .await?;
    }

    message
        .edit(
            ctx.serenity_context(),
            serenity::EditMessage::new().components(Vec::new()),
        )
        .await?;

    Ok(())
}

/// Find the best missions to farm a specific relic
#[poise::command(slash_command, prefix_command, category = "Warframe")]
pub async fn farm(
    ctx: Context<'_>,
    #[description = "Result page number (default 1)"]
    page: Option<usize>,
    #[description = "Relic to farm (e.g., 'Lith A1' or 'Axi S9')"]
    #[autocomplete = "farm_relic_autocomplete"]
    #[rest]
    relic: String,
) -> Result<(), Error> {
    ctx.defer().await?;

    let requested_page = page.unwrap_or(1).max(1);

    let Some(normalized_relic) = normalize_relic_name(relic.as_str()) else {
        ctx.say("❌ Invalid relic format. Use: 'Lith A1', 'Meso B5', 'Neo Z8', or 'Axi S9'")
            .await?;
        return Ok(());
    };

    let relic_name = format!("{} Relic", normalized_relic);
    let search_term = relic_name.as_str();

    let api_response: ApiResponse = match fetch_mission_rewards(&ctx).await {
        Ok(response) => response,
        Err(_) => {
            ctx.say("Failed to fetch mission data from API").await?;
            return Ok(());
        }
    };

    let mut found_drops = collect_found_drops(api_response, search_term);

    if found_drops.is_empty() {
        ctx.say(format!(
            "❌ No missions found that drop **{}**\n\nMake sure to use the format: 'Lith A1', 'Meso B5', 'Neo Z8', or 'Axi S9'. If the format is correct, this relic may not be in the current drop table.",
            relic_name
        ))
        .await?;
        return Ok(());
    }

    found_drops.sort_by(|a, b| b.chance.partial_cmp(&a.chance).unwrap_or(Ordering::Equal));

    let sorted_missions = group_and_sort_missions(found_drops);

    let pages = paginate_missions(sorted_missions, MISSION_PAGE_SIZE);

    let total_pages = pages.len();
    let mut current_page = requested_page.min(total_pages) - 1;

    let message = ctx
        .channel_id()
        .send_message(
            ctx.serenity_context(),
            serenity::CreateMessage::new()
                .content(format_page(
                    &relic_name,
                    current_page,
                    total_pages,
                    &pages[current_page],
                ))
                .components(page_components(current_page, total_pages)),
        )
        .await?;

    if total_pages > 1 {
        handle_page_interactions(
            ctx,
            message,
            &relic_name,
            total_pages,
            &pages,
            &mut current_page,
        )
        .await?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_reward(item_name: &str, rarity: &str, chance: f64) -> MissionReward {
        MissionReward {
            item_name: item_name.to_string(),
            rarity: rarity.to_string(),
            chance,
        }
    }

    #[test]
    fn test_normalize_relic_name() {
        assert_eq!(normalize_relic_name("lith a1"), Some("Lith A1".to_string()));
        assert_eq!(normalize_relic_name("MESO B5"), Some("Meso B5".to_string()));
        assert_eq!(normalize_relic_name("neo z8 relic"), Some("Neo Z8".to_string()));
        assert_eq!(normalize_relic_name("Axi"), None);
        assert_eq!(normalize_relic_name("Invalid Relic Name"), None);
    }

    #[test]
    fn test_relic_name_from_item() {
        assert_eq!(relic_name_from_item("Lith A1 Relic"), Some("Lith A1".to_string()));
        assert_eq!(relic_name_from_item("Axi S9 Relic"), Some("Axi S9".to_string()));
        assert_eq!(relic_name_from_item("Ash Prime Systems"), None);
    }

    #[test]
    fn test_for_each_mission_reward_supports_both_shapes() {
        let rotation_data = MissionData {
            game_mode: "Survival".to_string(),
            is_event: Some(false),
            rewards: MissionRewards::Rotations(HashMap::from([
                (
                    "A".to_string(),
                    vec![sample_reward("Lith A1 Relic", "Common", 10.0)],
                ),
                (
                    "B".to_string(),
                    vec![sample_reward("Meso B2 Relic", "Uncommon", 8.0)],
                ),
            ])),
        };

        let list_data = MissionData {
            game_mode: "Capture".to_string(),
            is_event: Some(false),
            rewards: MissionRewards::List(vec![sample_reward("Neo C3 Relic", "Rare", 5.0)]),
        };

        let mut seen = Vec::new();
        for_each_mission_reward(&rotation_data, |rotation, reward| {
            seen.push((rotation.to_string(), reward.item_name.clone()));
        });
        for_each_mission_reward(&list_data, |rotation, reward| {
            seen.push((rotation.to_string(), reward.item_name.clone()));
        });

        assert!(seen.contains(&("A".to_string(), "Lith A1 Relic".to_string())));
        assert!(seen.contains(&("B".to_string(), "Meso B2 Relic".to_string())));
        assert!(seen.contains(&("Any".to_string(), "Neo C3 Relic".to_string())));
    }

    #[test]
    fn test_collect_found_drops_skips_events() {
        let api_response = ApiResponse {
            mission_rewards: HashMap::from([(
                "Earth".to_string(),
                HashMap::from([
                    (
                        "E Prime".to_string(),
                        MissionData {
                            game_mode: "Capture".to_string(),
                            is_event: Some(false),
                            rewards: MissionRewards::List(vec![sample_reward(
                                "Lith A1 Relic",
                                "Common",
                                12.5,
                            )]),
                        },
                    ),
                    (
                        "Event Node".to_string(),
                        MissionData {
                            game_mode: "Defense".to_string(),
                            is_event: Some(true),
                            rewards: MissionRewards::List(vec![sample_reward(
                                "Lith A1 Relic",
                                "Rare",
                                1.0,
                            )]),
                        },
                    ),
                ]),
            )]),
        };

        let found = collect_found_drops(api_response, "lith a1 relic");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].mission_path, "Earth/E Prime");
    }
}
