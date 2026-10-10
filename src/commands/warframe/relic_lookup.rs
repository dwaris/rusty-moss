use super::api::get_cached_json;
use super::normalization::{normalize_text, normalize_whitespace};
use super::relic_farming::fetch_mission_rewards;
use super::relic_overlap::format_set_overlapping_missions;
use crate::{Context, Error};
use poise::serenity_prelude as serenity;
use std::cmp::Ordering;
use std::collections::HashSet;

use std::sync::Arc;

use super::types::{RelicResponse, RelicSearchQuery, RelicState, RelicTier, RELIC_STATES};

type RelicEntry = (String, Vec<FoundRelic>);

fn extract_prime_set_name(item_name: &str) -> Option<String> {
    let tokens: Vec<&str> = item_name.split_whitespace().collect();
    let prime_index = tokens
        .iter()
        .position(|token| token.eq_ignore_ascii_case("prime"))?;

    if prime_index == 0 {
        return None;
    }

    Some(tokens[..=prime_index].join(" "))
}

fn part_sort_key(part_name: &str) -> (usize, String) {
    let normalized = normalize_text(part_name);
    let rank = if normalized.ends_with(" prime blueprint") {
        0
    } else if normalized.ends_with(" prime chassis blueprint") {
        1
    } else if normalized.ends_with(" prime neuroptics blueprint") {
        2
    } else if normalized.ends_with(" prime systems blueprint") {
        3
    } else {
        10
    };

    (rank, normalized)
}

fn collect_item_suggestions(relics: &RelicResponse, partial: &str) -> Vec<String> {
    let partial_lower = normalize_text(partial);
    let mut unique = HashSet::new();

    for relic in &relics.relics {
        for reward in &relic.rewards {
            let item_normalized = normalize_text(&reward.item_name);
            if !item_normalized.contains("prime") {
                continue;
            }

            let cleaned = normalize_whitespace(&reward.item_name);
            if partial_lower.is_empty() || item_normalized.contains(&partial_lower) {
                unique.insert(cleaned);
            }

            if let Some(set_name) = extract_prime_set_name(&reward.item_name) {
                let set_normalized = normalize_text(&set_name);
                if partial_lower.is_empty() || set_normalized.contains(&partial_lower) {
                    unique.insert(normalize_whitespace(&set_name));
                }
            }
        }
    }

    let mut suggestions: Vec<String> = unique.into_iter().collect();
    suggestions.sort();
    suggestions.truncate(25);
    suggestions
}

async fn relic_item_autocomplete(ctx: Context<'_>, partial: &str) -> serenity::CreateAutocompleteResponse {
    let relics = match fetch_relics(&ctx).await {
        Ok(relics) => relics,
        Err(_) => return serenity::CreateAutocompleteResponse::new(),
    };

    let choices = collect_item_suggestions(&relics, partial)
        .into_iter()
        .map(serenity::AutocompleteChoice::from)
        .collect();

    serenity::CreateAutocompleteResponse::new().set_choices(choices)
}

#[derive(Debug)]
struct FoundRelic {
    tier: RelicTier,
    relic_name: String,
    state: RelicState,
    chance: f64,
}

async fn fetch_relics(ctx: &Context<'_>) -> Result<Arc<RelicResponse>, Error> {
    get_cached_json(ctx, "https://drops.warframestat.us/data/relics.json").await
}

fn collect_matching_relics(relics: &RelicResponse, search_term: &str) -> Vec<FoundRelic> {
    let mut found = Vec::new();

    for relic in &relics.relics {
        let Some(relic_name_part) = relic.relic_name.as_deref() else {
            continue;
        };

        let relic_name = format!("{} {}", relic.tier, relic_name_part);
        for reward in &relic.rewards {
            if normalize_text(&reward.item_name).contains(search_term) {
                found.push(FoundRelic {
                    tier: relic.tier,
                    relic_name: relic_name.clone(),
                    state: relic.state,
                    chance: reward.chance,
                });
            }
        }
    }

    found
}

fn collect_set_parts(relics: &RelicResponse, set_name: &str) -> Vec<String> {
    let set_prefix = format!("{} ", normalize_text(set_name));
    let mut unique_parts = HashSet::new();

    for relic in &relics.relics {
        for reward in &relic.rewards {
            let normalized_item = normalize_text(&reward.item_name);

            if normalized_item.starts_with(&set_prefix) {
                unique_parts.insert(normalize_whitespace(&reward.item_name));
            }
        }
    }

    let mut parts: Vec<String> = unique_parts.into_iter().collect();
    parts.sort_by_key(|part| part_sort_key(part));
    parts
}

fn group_sorted_relics(relics: &RelicResponse, search_term: &str) -> Vec<RelicEntry> {
    let mut found_relics = collect_matching_relics(relics, search_term);
    sort_relics(&mut found_relics);
    group_relics(found_relics)
}

fn format_state_chances(entries: &[FoundRelic]) -> String {
    let mut state_parts = Vec::new();

    for state in RELIC_STATES {
        if let Some(entry) = entries.iter().find(|entry| entry.state == state) {
            state_parts.push(format!("{state} {:.2}%", entry.chance));
        }
    }

    state_parts.join(" | ")
}

fn create_relic_embed(item: &str, grouped_relics: &[RelicEntry]) -> serenity::CreateEmbed {
    let embed = serenity::CreateEmbed::new()
        .title(format!("Relics containing {}", item))
        .color(0x00AE86);

    let mut description = String::new();
    for (relic_name, entries) in grouped_relics {
        description.push_str(&format!("**{}**\n", relic_name));
        description.push_str(&format!("└ {}\n\n", format_state_chances(entries)));
    }

    if description.is_empty() {
        description.push_str("No relics found.");
    }

    embed.description(description)
}

fn create_set_embeds(
    set_name: &str,
    part_groups: &[(String, Vec<RelicEntry>)],
    overlap_summary: Option<&str>,
) -> Vec<serenity::CreateEmbed> {
    let mut embeds = Vec::new();
    let mut current_embed = serenity::CreateEmbed::new()
        .title(format!("Relics for {} set", set_name))
        .color(0x00AE86);

    let mut current_description = String::new();

    for (part_name, grouped_relics) in part_groups {
        let mut part_text = format!("__**{}**__\n", part_name);

        if grouped_relics.is_empty() {
            part_text.push_str("  No relics found for this part.\n\n");
        } else {
            for (relic_name, entries) in grouped_relics {
                part_text.push_str(&format!(
                    "**{}** -> {}\n",
                    relic_name,
                    format_state_chances(entries)
                ));
            }
            part_text.push('\n');
        }

        if current_description.len() + part_text.len() > 3800 {
            embeds.push(current_embed.description(current_description));
            current_embed = serenity::CreateEmbed::new().color(0x00AE86);
            current_description = String::new();
        }

        current_description.push_str(&part_text);
    }

    if let Some(overlap_text) = overlap_summary {
        let section = format!("__**Best Overlapping Farm Spots**__\n{}\n", overlap_text);
        if current_description.len() + section.len() > 3800 {
            embeds.push(current_embed.description(current_description));
            current_embed = serenity::CreateEmbed::new()
                .title(format!("Best Overlapping Farm Spots for {} set", set_name))
                .color(0x00AE86);
            current_description = section;
        } else {
            current_description.push_str(&section);
        }
    }

    if !current_description.is_empty() {
        embeds.push(current_embed.description(current_description));
    }

    embeds
}

async fn send_embeds(ctx: Context<'_>, embeds: Vec<serenity::CreateEmbed>) -> Result<(), Error> {
    if embeds.is_empty() {
        return Ok(());
    }

    for embed in embeds {
        ctx.send(poise::CreateReply::default().embed(embed)).await?;
    }

    Ok(())
}

fn sort_relics(found_relics: &mut [FoundRelic]) {
    found_relics.sort_by(|a, b| {
        a.tier
            .cmp(&b.tier)
            .then_with(|| a.relic_name.cmp(&b.relic_name))
            .then_with(|| a.state.cmp(&b.state))
            .then_with(|| b.chance.partial_cmp(&a.chance).unwrap_or(Ordering::Equal))
    });
}

fn group_relics(found_relics: Vec<FoundRelic>) -> Vec<RelicEntry> {
    let mut grouped: Vec<RelicEntry> = Vec::new();

    for relic in found_relics {
        if let Some((last_name, entries)) = grouped.last_mut()
            && *last_name == relic.relic_name
        {
            entries.push(relic);
            continue;
        }

        grouped.push((relic.relic_name.clone(), vec![relic]));
    }

    grouped
}

async fn handle_set_query(
    ctx: Context<'_>,
    relics: &RelicResponse,
    set_name: &str,
) -> Result<(), Error> {
    let set_parts = collect_set_parts(relics, set_name);

    if set_parts.is_empty() {
        ctx.say(format!(
            "No Prime set parts found for {}. Try a full Prime set name like 'Xaku Prime'.",
            set_name
        ))
        .await?;
        return Ok(());
    }

    let mut part_groups = Vec::new();
    let mut set_relic_names = HashSet::new();

    for part in set_parts {
        let grouped_relics = group_sorted_relics(relics, &normalize_text(&part));
        for (relic_name, _) in &grouped_relics {
            set_relic_names.insert(relic_name.clone());
        }
        part_groups.push((part, grouped_relics));
    }

    let mission_api = fetch_mission_rewards(&ctx).await.ok();
    let overlap_summary = mission_api.as_deref().map(|api| {
        format_set_overlapping_missions(api, &set_relic_names, 4)
    });

    let embeds = create_set_embeds(set_name, &part_groups, overlap_summary.as_deref());
    send_embeds(ctx, embeds).await
}

async fn handle_single_item_query(
    ctx: Context<'_>,
    relics: &RelicResponse,
    item: &str,
) -> Result<(), Error> {
    let grouped_relics = group_sorted_relics(relics, &normalize_text(item));

    if grouped_relics.is_empty() {
        ctx.say(format!("No relics found containing {}\n\nMake sure to include 'Prime' in your search (e.g., 'Ash Prime Systems')", item))
            .await?;
        return Ok(());
    }

    let embed = create_relic_embed(item, &grouped_relics);
    ctx.send(poise::CreateReply::default().embed(embed)).await?;
    Ok(())
}

/// Find which relics contain a specific prime item
#[poise::command(
    slash_command,
    prefix_command,
    category = "Warframe",
    rename = "lookup"
)]
pub async fn lookup(
    ctx: Context<'_>,
    #[description = "Prime item to search for (e.g., 'Ash Prime Systems')"]
    #[autocomplete = "relic_item_autocomplete"]
    #[rest]
    item: String,
) -> Result<(), Error> {
    ctx.defer().await?;

    let relics = match fetch_relics(&ctx).await {
        Ok(relics) => relics,
        Err(e) => {
            log::error!("Failed to fetch relic data from API: {e}");
            ctx.say("Failed to fetch relic data from API").await?;
            return Ok(());
        }
    };

    match RelicSearchQuery::parse(&item) {
        RelicSearchQuery::PrimeSet(set_name) => handle_set_query(ctx, &relics, &set_name).await,
        RelicSearchQuery::SpecificItem(item_name) => {
            handle_single_item_query(ctx, &relics, &item_name).await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::types::{Relic, RelicReward};

    #[test]
    fn test_extract_prime_set_name() {
        assert_eq!(
            extract_prime_set_name("Ash Prime Systems"),
            Some("Ash Prime".to_string())
        );
        assert_eq!(
            extract_prime_set_name("Ash Prime"),
            Some("Ash Prime".to_string())
        );
        assert_eq!(
            extract_prime_set_name("Braton Prime Barrel"),
            Some("Braton Prime".to_string())
        );
        assert_eq!(extract_prime_set_name("Forma BP"), None);
        assert_eq!(extract_prime_set_name("Prime Part"), None);
    }

    #[test]
    fn test_part_sort_key() {
        let bp = part_sort_key("Ash Prime Blueprint");
        let chassis = part_sort_key("Ash Prime Chassis Blueprint");
        let systems = part_sort_key("Ash Prime Systems Blueprint");
        let neuro = part_sort_key("Ash Prime Neuroptics Blueprint");
        let other = part_sort_key("Ash Prime Stock");

        assert!(bp.0 < chassis.0);
        assert!(chassis.0 < neuro.0);
        assert!(neuro.0 < systems.0);
        assert!(systems.0 < other.0);
    }

    #[test]
    fn test_format_state_chances_orders_by_known_state_priority() {
        let entries = vec![
            FoundRelic {
                tier: RelicTier::Lith,
                relic_name: "Lith A1".to_string(),
                state: RelicState::Radiant,
                chance: 10.0,
            },
            FoundRelic {
                tier: RelicTier::Lith,
                relic_name: "Lith A1".to_string(),
                state: RelicState::Intact,
                chance: 2.0,
            },
        ];

        assert_eq!(
            format_state_chances(&entries),
            "Intact 2.00% | Radiant 10.00%"
        );
    }

    #[test]
    fn test_group_sorted_relics_groups_same_relic_name() {
        let relics = RelicResponse {
            relics: vec![
                Relic {
                    _id: "1".to_string(),
                    tier: RelicTier::Lith,
                    relic_name: Some("A1".to_string()),
                    state: RelicState::Intact,
                    rewards: vec![RelicReward {
                        item_name: "Ash Prime Systems".to_string(),
                        chance: 2.0,
                    }],
                },
                Relic {
                    _id: "2".to_string(),
                    tier: RelicTier::Lith,
                    relic_name: Some("A1".to_string()),
                    state: RelicState::Radiant,
                    rewards: vec![RelicReward {
                        item_name: "Ash Prime Systems".to_string(),
                        chance: 10.0,
                    }],
                },
            ],
        };

        let grouped = group_sorted_relics(&relics, "ash prime systems");
        assert_eq!(grouped.len(), 1);
        assert_eq!(grouped[0].0, "Lith A1");
        assert_eq!(grouped[0].1.len(), 2);
    }

    #[test]
    fn test_relic_search_query_parse() {
        assert_eq!(
            RelicSearchQuery::parse("Ash Prime Set"),
            RelicSearchQuery::PrimeSet("Ash Prime".to_string())
        );
        assert_eq!(
            RelicSearchQuery::parse("Ash Prime Systems"),
            RelicSearchQuery::SpecificItem("Ash Prime Systems".to_string())
        );
    }

    #[test]
    fn test_create_set_embeds_includes_overlap_section() {
        let part_groups = vec![(
            "Ash Prime Blueprint".to_string(),
            vec![(
                "Lith A1".to_string(),
                vec![FoundRelic {
                    tier: RelicTier::Lith,
                    relic_name: "Lith A1".to_string(),
                    state: RelicState::Intact,
                    chance: 2.0,
                }],
            )],
        )];

        let overlap_text = "**Lua/Apollo (Disruption)** — **2 relics covered**\n";
        let embeds = create_set_embeds("Ash Prime", &part_groups, Some(overlap_text));

        assert_eq!(embeds.len(), 1);
        let json = serde_json::to_value(&embeds[0]).unwrap();
        let desc = json["description"].as_str().unwrap_or("");
        assert!(desc.contains("Ash Prime Blueprint"));
        assert!(desc.contains("Lith A1"));
        assert!(desc.contains("Best Overlapping Farm Spots"));
        assert!(desc.contains("Lua/Apollo (Disruption)"));
    }
}
