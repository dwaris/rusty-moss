use super::api::get_cached_json;
use crate::{Context, Error};
use serde::Deserialize;
use std::cmp::Ordering;
use std::collections::HashSet;

#[derive(Deserialize, Debug)]
struct RelicResponse {
    relics: Vec<Relic>,
}

#[derive(Deserialize, Debug)]
struct Relic {
    #[serde(rename = "_id")]
    _id: String,
    tier: String,
    #[serde(rename = "relicName", alias = "name", default)]
    relic_name: Option<String>,
    state: String,
    rewards: Vec<RelicReward>,
}

#[derive(Deserialize, Debug)]
struct RelicReward {
    #[serde(rename = "itemName")]
    item_name: String,
    chance: f64,
}

type RelicEntry = (String, Vec<FoundRelic>);

fn normalize_text(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn collect_item_suggestions(relics: &RelicResponse, partial: &str) -> Vec<String> {
    let partial_lower = normalize_text(partial);
    let mut unique = HashSet::new();

    for relic in &relics.relics {
        for reward in &relic.rewards {
            let cleaned = reward.item_name.split_whitespace().collect::<Vec<_>>().join(" ");
            if !cleaned.contains("Prime") {
                continue;
            }

            if partial_lower.is_empty() || cleaned.to_lowercase().contains(&partial_lower) {
                unique.insert(cleaned);
            }
        }
    }

    let mut suggestions: Vec<String> = unique.into_iter().collect();
    suggestions.sort();
    suggestions.truncate(25);
    suggestions
}

async fn relic_item_autocomplete(ctx: Context<'_>, partial: &str) -> Vec<String> {
    let relics = match fetch_relics(&ctx).await {
        Ok(relics) => relics,
        Err(_) => return Vec::new(),
    };

    collect_item_suggestions(&relics, partial)
}

fn state_order(state: &str) -> usize {
    match state {
        "Intact" => 0,
        "Exceptional" => 1,
        "Flawless" => 2,
        "Radiant" => 3,
        _ => 4,
    }
}

#[derive(Debug)]
struct FoundRelic {
    tier: String,
    relic_name: String,
    state: String,
    chance: f64,
}

async fn fetch_relics(ctx: &Context<'_>) -> Result<RelicResponse, Error> {
    let payload = get_cached_json(ctx, "https://drops.warframestat.us/data/relics.json").await?;
    Ok(serde_json::from_value(payload)?)
}

fn collect_matching_relics(relics: RelicResponse, search_term: &str) -> Vec<FoundRelic> {
    let mut found = Vec::new();

    for relic in relics.relics {
        let Some(relic_name_part) = relic.relic_name.as_deref() else {
            continue;
        };

        let relic_name = format!("{} {}", relic.tier, relic_name_part);
        for reward in &relic.rewards {
            if normalize_text(&reward.item_name).contains(search_term) {
                found.push(FoundRelic {
                    tier: relic.tier.clone(),
                    relic_name: relic_name.clone(),
                    state: relic.state.clone(),
                    chance: reward.chance,
                });
            }
        }
    }

    found
}

fn sort_relics(found_relics: &mut [FoundRelic]) {
    found_relics.sort_by(|a, b| {
        a.tier
            .cmp(&b.tier)
            .then_with(|| a.relic_name.cmp(&b.relic_name))
            .then_with(|| state_order(&a.state).cmp(&state_order(&b.state)))
            .then_with(|| b.chance.partial_cmp(&a.chance).unwrap_or(Ordering::Equal))
    });
}

fn group_relics(found_relics: Vec<FoundRelic>) -> Vec<RelicEntry> {
    let mut grouped: Vec<RelicEntry> = Vec::new();

    for relic in found_relics {
        if let Some((last_name, entries)) = grouped.last_mut() {
            if *last_name == relic.relic_name {
                entries.push(relic);
                continue;
            }
        }

        grouped.push((relic.relic_name.clone(), vec![relic]));
    }

    grouped
}

fn format_relic_response(item: &str, grouped_relics: &[RelicEntry]) -> String {
    let mut response_text = format!("Relics containing {}:\n\n", item);

    for (relic_name, entries) in grouped_relics {
        response_text.push_str(&format!("{}\n", relic_name));

        let states = ["Intact", "Exceptional", "Flawless", "Radiant"];
        let mut state_parts = Vec::new();

        for state in states {
            if let Some(entry) = entries.iter().find(|e| e.state == state) {
                state_parts.push(format!("{} {:.2}%", state, entry.chance));
            }
        }

        response_text.push_str(&format!("  {}\n", state_parts.join(" | ")));

        response_text.push('\n');
    }

    response_text
}

/// Find which relics contain a specific prime item
#[poise::command(slash_command, prefix_command, category = "Warframe")]
pub async fn relic(
    ctx: Context<'_>,
    #[description = "Prime item to search for (e.g., 'Ash Prime Systems')"]
    #[autocomplete = "relic_item_autocomplete"]
    #[rest]
    item: String,
) -> Result<(), Error> {
    ctx.defer().await?;

    let relics = match fetch_relics(&ctx).await {
        Ok(relics) => relics,
        Err(_) => {
            ctx.say("Failed to fetch relic data from API").await?;
            return Ok(());
        }
    };

    let search_term = normalize_text(&item);
    let mut found_relics = collect_matching_relics(relics, &search_term);

    if found_relics.is_empty() {
        ctx.say(format!("No relics found containing {}\n\nMake sure to include 'Prime' in your search (e.g., 'Ash Prime Systems')", item))
            .await?;
        return Ok(());
    }

    sort_relics(&mut found_relics);
    let grouped_relics = group_relics(found_relics);
    let response_text = format_relic_response(&item, &grouped_relics);

    ctx.say(response_text).await?;
    Ok(())
}
