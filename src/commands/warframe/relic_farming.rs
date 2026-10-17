use super::api::get_cached_json;
use super::normalization::{normalize_whitespace, starts_with_ignore_case};
use crate::{Context, Error};
use poise::serenity_prelude as serenity;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use super::types::{
    MissionApiResponse, MissionData, MissionDrops, MissionReward, MissionRewards, RelicTier,
    RotationDrop,
};

const MISSION_PAGE_SIZE: usize = 10;

const EFFICIENT_GAME_MODES: &[&str] = &["Capture", "Disruption", "Defense"];

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

    let (tier_str, code) = match parts.as_slice() {
        [tier, code] => (*tier, *code),
        [tier, code, suffix] if suffix.eq_ignore_ascii_case("relic") => (*tier, *code),
        _ => return None,
    };

    let tier = RelicTier::parse(tier_str)?;
    Some(format!("{} {}", tier, code.to_ascii_uppercase()))
}

pub(crate) fn relic_name_from_item(item_name: &str) -> Option<String> {
    let cleaned = normalize_whitespace(item_name);
    let without_parentheses = if let Some(idx) = cleaned.find(" Relic (") {
        &cleaned[..idx]
    } else {
        cleaned.strip_suffix(" Relic")?
    };
    Some(without_parentheses.to_string())
}

fn relic_tier_and_code(name: &str) -> Option<(&str, &str)> {
    let mut parts = name.split_whitespace();
    let tier = parts.next()?;
    let code = parts.next()?;
    Some((tier, code))
}

fn is_subsequence(sub: &str, text: &str) -> bool {
    let mut text_chars = text.chars();
    for s in sub.chars() {
        if !text_chars.any(|c| c.eq_ignore_ascii_case(&s)) {
            return false;
        }
    }
    true
}

fn fuzzy_match_relic_score(name: &str, query: &str) -> Option<u32> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return Some(100);
    }

    let n_lower = name.to_lowercase();
    if n_lower == q {
        return Some(0);
    }

    let (_tier, code) = relic_tier_and_code(name).unwrap_or(("", ""));
    let code_lower = code.to_lowercase();

    if code_lower == q {
        return Some(10);
    }

    if n_lower.starts_with(&q) {
        return Some(20);
    }

    if code_lower.starts_with(&q) {
        return Some(30);
    }

    if code_lower.contains(&q) {
        return Some(40);
    }

    let q_tokens: Vec<&str> = q.split_whitespace().collect();
    if q_tokens.len() > 1 && q_tokens.iter().all(|tok| n_lower.contains(tok)) {
        return Some(50);
    }

    if n_lower.contains(&q) {
        return Some(60);
    }

    let q_condensed: String = q.chars().filter(|c| !c.is_whitespace()).collect();
    let n_condensed: String = n_lower.chars().filter(|c| !c.is_whitespace()).collect();
    if !q_condensed.is_empty() && is_subsequence(&q_condensed, &n_condensed) {
        return Some(70);
    }

    None
}

pub fn filter_relic_suggestions<'a>(
    names: impl IntoIterator<Item = &'a String>,
    query: &str,
) -> Vec<String> {
    let query_trimmed = query.trim();
    if query_trimmed.is_empty() {
        let mut list: Vec<String> = names.into_iter().cloned().collect();
        list.sort();
        list.truncate(25);
        return list;
    }

    let mut matches: Vec<(u32, String)> = names
        .into_iter()
        .filter_map(|name| {
            fuzzy_match_relic_score(name, query_trimmed).map(|score| (score, name.clone()))
        })
        .collect();

    matches.sort_by(|(score_a, name_a), (score_b, name_b)| {
        score_a.cmp(score_b).then_with(|| name_a.cmp(name_b))
    });

    matches.into_iter().map(|(_, name)| name).take(25).collect()
}

pub(crate) fn for_each_mission_reward<F>(mission_data: &MissionData, mut callback: F)
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

pub(crate) async fn fetch_mission_rewards(
    ctx: &Context<'_>,
) -> Result<Arc<MissionApiResponse>, Error> {
    get_cached_json(
        ctx,
        "https://drops.warframestat.us/data/missionRewards.json",
    )
    .await
}

fn collect_relic_name_suggestions(api_response: &MissionApiResponse) -> Vec<String> {
    let mut unique = HashSet::new();

    for missions in api_response.mission_rewards.values() {
        for mission_data in missions.values() {
            for_each_mission_reward(mission_data, |_, reward| {
                if let Some(name) = relic_name_from_item(&reward.item_name)
                    && let Some(normalized) = normalize_relic_name(&name)
                {
                    unique.insert(normalized);
                }
            });
        }
    }

    let mut suggestions: Vec<String> = unique.into_iter().collect();
    suggestions.sort();
    suggestions
}

async fn farm_relic_autocomplete(
    ctx: Context<'_>,
    partial: &str,
) -> serenity::CreateAutocompleteResponse {
    const RELIC_NAMES_KEY: &str = "warframe:relic_names";

    let cached: Option<Arc<Vec<String>>> =
        if let Some(cached) = ctx.data().warframe_cache.get(RELIC_NAMES_KEY).await {
            cached.downcast::<Vec<String>>().ok()
        } else {
            None
        };

    let names = if let Some(names) = cached {
        names
    } else {
        let api_response = match fetch_mission_rewards(&ctx).await {
            Ok(response) => response,
            Err(_) => return serenity::CreateAutocompleteResponse::new(),
        };
        let names: Arc<Vec<String>> = Arc::new(collect_relic_name_suggestions(&api_response));
        ctx.data()
            .warframe_cache
            .insert(RELIC_NAMES_KEY.to_string(), names.clone())
            .await;
        names
    };

    let choices = filter_relic_suggestions(names.iter(), partial)
        .into_iter()
        .map(serenity::AutocompleteChoice::from)
        .collect();

    serenity::CreateAutocompleteResponse::new().set_choices(choices)
}

fn page_components(current_page: usize, total_pages: usize) -> Vec<serenity::CreateActionRow> {
    let prev_button = serenity::CreateButton::new("farm_prev")
        .label("Previous")
        .style(serenity::ButtonStyle::Secondary)
        .disabled(current_page == 0);

    let next_button = serenity::CreateButton::new("farm_next")
        .label("Next")
        .style(serenity::ButtonStyle::Secondary)
        .disabled(current_page + 1 >= total_pages);

    vec![serenity::CreateActionRow::Buttons(vec![
        prev_button,
        next_button,
    ])]
}

fn collect_found_drops(
    api_response: &MissionApiResponse,
    search_term: &str,
    fast_only: bool,
) -> Vec<FoundDrop> {
    let mut found = Vec::new();

    for (planet, missions) in &api_response.mission_rewards {
        for (mission_name, mission_data) in missions {
            if mission_data.is_event.unwrap_or(false) {
                continue;
            }

            if fast_only && !EFFICIENT_GAME_MODES.contains(&mission_data.game_mode.as_str()) {
                continue;
            }

            let mission_path = format!("{}/{}", planet, mission_name);

            for_each_mission_reward(mission_data, |rotation, reward| {
                if !starts_with_ignore_case(&reward.item_name, search_term) {
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
        let mission_label = format!("{} ({})", drop.mission_path, drop.game_mode);
        mission_map
            .entry(mission_label)
            .or_default()
            .push(RotationDrop {
                rotation: drop.rotation,
                rarity: drop.rarity,
                chance: drop.chance,
            });
    }

    for drops in mission_map.values_mut() {
        drops.sort_by(|a, b| b.chance.partial_cmp(&a.chance).unwrap_or(Ordering::Equal));
    }

    let mut missions: Vec<MissionDrops> = mission_map
        .into_iter()
        .map(|(mission_label, drops)| MissionDrops {
            mission_label,
            drops,
        })
        .collect();

    missions.sort_by(|a, b| {
        let best_a = a.drops.iter().map(|x| x.chance).fold(0.0, f64::max);
        let best_b = b.drops.iter().map(|x| x.chance).fold(0.0, f64::max);
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

fn create_page_embed(
    relic_name: &str,
    current_page: usize,
    total_pages: usize,
    missions: &[MissionDrops],
) -> serenity::CreateEmbed {
    let embed = serenity::CreateEmbed::new()
        .title(format!("Best missions to farm {}", relic_name))
        .color(0x00AE86)
        .footer(serenity::CreateEmbedFooter::new(format!(
            "Page {}/{}",
            current_page + 1,
            total_pages
        )));

    let mut description = String::new();
    for mission in missions {
        description.push_str(&format!("**{}**\n", mission.mission_label));
        for drop in &mission.drops {
            description.push_str(&format!(
                "└ Rotation {} - {}: **{:.2}%**\n",
                drop.rotation, drop.rarity, drop.chance
            ));
        }
        description.push('\n');
    }

    if description.is_empty() {
        description.push_str("No missions found.");
    }

    embed.description(description)
}

async fn handle_page_interactions(
    ctx: Context<'_>,
    mut message: serenity::Message,
    relic_name: &str,
    total_pages: usize,
    pages: &[Vec<MissionDrops>],
    current_page: &mut usize,
) -> Result<(), Error> {
    while let Some(interaction) =
        serenity::collector::ComponentInteractionCollector::new(ctx.serenity_context())
            .author_id(ctx.author().id)
            .channel_id(ctx.channel_id())
            .message_id(message.id)
            .timeout(Duration::from_secs(120))
            .await
    {
        match interaction.data.custom_id.as_str() {
            "farm_prev" if *current_page > 0 => *current_page -= 1,
            "farm_next" if *current_page + 1 < total_pages => *current_page += 1,
            _ => continue,
        }

        interaction
            .create_response(
                ctx.serenity_context(),
                serenity::CreateInteractionResponse::UpdateMessage(
                    serenity::CreateInteractionResponseMessage::new()
                        .add_embed(create_page_embed(
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
    #[description = "Only show efficient mission types (default true)"] fast_only: Option<bool>,
    #[description = "Result page number (default 1)"] page: Option<u32>,
    #[description = "Relic to farm (e.g., 'Lith A1' or 'Axi S9')"]
    #[autocomplete = "farm_relic_autocomplete"]
    #[rest]
    relic: String,
) -> Result<(), Error> {
    ctx.defer().await?;

    let fast_only = fast_only.unwrap_or(true);
    let requested_page = (page.unwrap_or(1).max(1)) as usize;

    let Some(normalized_relic) = normalize_relic_name(relic.as_str()) else {
        ctx.say("Invalid relic format. Use: 'Lith A1', 'Meso B5', 'Neo Z8', or 'Axi S9'")
            .await?;
        return Ok(());
    };

    let relic_name = format!("{} Relic", normalized_relic);
    let search_term = relic_name.as_str();

    let api_response = match fetch_mission_rewards(&ctx).await {
        Ok(response) => response,
        Err(e) => {
            log::error!("Failed to fetch mission data from API: {e}");
            ctx.say("Failed to fetch mission data from API").await?;
            return Ok(());
        }
    };

    let mut found_drops = collect_found_drops(&api_response, search_term, fast_only);

    if found_drops.is_empty() {
        let msg = if fast_only {
            format!(
                "No efficient missions found that drop **{}**. Try setting `fast_only` to false.",
                relic_name
            )
        } else {
            format!(
                "No missions found that drop **{}**\n\nMake sure to use the format: 'Lith A1', 'Meso B5', 'Neo Z8', or 'Axi S9'. If the format is correct, this relic may not be in the current drop table.",
                relic_name
            )
        };
        ctx.say(msg).await?;
        return Ok(());
    }

    found_drops.sort_by(|a, b| b.chance.partial_cmp(&a.chance).unwrap_or(Ordering::Equal));

    let sorted_missions = group_and_sort_missions(found_drops);

    let pages = paginate_missions(sorted_missions, MISSION_PAGE_SIZE);

    let total_pages = pages.len();
    let mut current_page = (requested_page - 1).min(total_pages - 1);

    // Disable pagination by default if filtered, unless explicitly requested via page parameter
    let use_pagination = !fast_only || page.is_some();

    let mut reply = poise::CreateReply::default().embed(create_page_embed(
        &relic_name,
        current_page,
        total_pages,
        &pages[current_page],
    ));

    if use_pagination && total_pages > 1 {
        reply = reply.components(page_components(current_page, total_pages));
    }

    let message = ctx.send(reply).await?.into_message().await?;

    if use_pagination && total_pages > 1 {
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
        assert_eq!(
            normalize_relic_name("neo z8 relic"),
            Some("Neo Z8".to_string())
        );
        assert_eq!(normalize_relic_name("Axi"), None);
        assert_eq!(normalize_relic_name("Invalid Relic Name"), None);
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
        let api_response = MissionApiResponse {
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

        let found = collect_found_drops(&api_response, "lith a1 relic", false);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].mission_path, "Earth/E Prime");
    }

    #[test]
    fn test_group_and_sort_missions() {
        let drops = vec![
            FoundDrop {
                mission_path: "Earth/E Prime".to_string(),
                game_mode: "Capture".to_string(),
                rotation: "A".to_string(),
                rarity: "Common".to_string(),
                chance: 5.0,
            },
            FoundDrop {
                mission_path: "Mars/O Olympus".to_string(),
                game_mode: "Disruption".to_string(),
                rotation: "B".to_string(),
                rarity: "Uncommon".to_string(),
                chance: 14.0,
            },
            FoundDrop {
                mission_path: "Earth/E Prime".to_string(),
                game_mode: "Capture".to_string(),
                rotation: "B".to_string(),
                rarity: "Uncommon".to_string(),
                chance: 10.0,
            },
        ];

        let grouped = group_and_sort_missions(drops);
        assert_eq!(grouped.len(), 2);
        // Mars should be first because its best chance is 14.0 > 10.0
        assert_eq!(grouped[0].mission_label, "Mars/O Olympus (Disruption)");
        assert_eq!(grouped[0].drops[0].chance, 14.0);

        // Earth should have its drops sorted descending (10.0 before 5.0)
        assert_eq!(grouped[1].mission_label, "Earth/E Prime (Capture)");
        assert_eq!(grouped[1].drops[0].chance, 10.0);
        assert_eq!(grouped[1].drops[1].chance, 5.0);
    }

    #[test]
    fn test_relic_name_from_item_handles_radiant() {
        assert_eq!(
            relic_name_from_item("Axi A21 Relic (Radiant)"),
            Some("Axi A21".to_string())
        );
        assert_eq!(
            relic_name_from_item("Lith T11 Relic"),
            Some("Lith T11".to_string())
        );
    }

    #[test]
    fn test_filter_relic_suggestions_fuzzy_and_numeric() {
        let relics = vec![
            "Axi A11".to_string(),
            "Axi A21".to_string(),
            "Lith T11".to_string(),
            "Meso B5".to_string(),
            "Neo V11".to_string(),
        ];

        // "11" should find all relics with 11 in their code
        let matches_11 = filter_relic_suggestions(&relics, "11");
        assert_eq!(matches_11, vec!["Axi A11", "Lith T11", "Neo V11"]);

        // "a11" should prioritize Axi A11
        let matches_a11 = filter_relic_suggestions(&relics, "a11");
        assert_eq!(matches_a11[0], "Axi A11");

        // "axi 11" should match Axi A11
        let matches_axi_11 = filter_relic_suggestions(&relics, "axi 11");
        assert_eq!(matches_axi_11, vec!["Axi A11"]);

        // "axi" should match all Axi relics
        let matches_axi = filter_relic_suggestions(&relics, "axi");
        assert_eq!(matches_axi, vec!["Axi A11", "Axi A21"]);
    }
}
