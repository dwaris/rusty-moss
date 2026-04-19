use super::api::get_cached_json;
use super::normalization::{normalize_text, normalize_whitespace};
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
const RELIC_STATES: [&str; 4] = ["Intact", "Exceptional", "Flawless", "Radiant"];

fn extract_prime_set_name(item_name: &str) -> Option<String> {
    let tokens: Vec<&str> = item_name.split_whitespace().collect();
    let prime_index = tokens.iter().position(|token| token.eq_ignore_ascii_case("prime"))?;

    if prime_index == 0 {
        return None;
    }

    Some(tokens[..=prime_index].join(" "))
}

fn parse_prime_set_query(input: &str) -> Option<String> {
    let mut tokens: Vec<&str> = input.split_whitespace().collect();
    if tokens.is_empty() {
        return None;
    }

    if tokens
        .last()
        .map(|token| token.eq_ignore_ascii_case("set"))
        .unwrap_or(false)
    {
        tokens.pop();
    }

    if tokens.is_empty() {
        return None;
    }

    let prime_index = tokens.iter().position(|token| token.eq_ignore_ascii_case("prime"))?;
    if prime_index == 0 || prime_index != tokens.len() - 1 {
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
            let cleaned = normalize_whitespace(&reward.item_name);
            if !cleaned.contains("Prime") {
                continue;
            }

            let cleaned_lower = cleaned.to_lowercase();
            if partial_lower.is_empty() || cleaned_lower.contains(&partial_lower) {
                unique.insert(cleaned);
            }

            if let Some(set_name) = extract_prime_set_name(&reward.item_name) {
                let cleaned_set = normalize_whitespace(&set_name);
                if partial_lower.is_empty() || cleaned_set.to_lowercase().contains(&partial_lower) {
                    unique.insert(cleaned_set);
                }
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

fn collect_set_parts(relics: &RelicResponse, set_name: &str) -> Vec<String> {
    let set_prefix = format!("{} ", normalize_text(set_name));
    let mut unique_parts = HashSet::new();

    for relic in &relics.relics {
        for reward in &relic.rewards {
            let cleaned_item = normalize_whitespace(&reward.item_name);
            let normalized_item = cleaned_item.to_lowercase();

            if normalized_item.starts_with(&set_prefix) {
                unique_parts.insert(cleaned_item);
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
            state_parts.push(format!("{} {:.2}%", state, entry.chance));
        }
    }

    state_parts.join(" | ")
}

fn format_set_response(set_name: &str, part_groups: &[(String, Vec<RelicEntry>)]) -> String {
    let mut response_text = format!("Relics for {} set:\n\n", set_name);

    for (part_name, grouped_relics) in part_groups {
        response_text.push_str(&format!("{}\n", part_name));

        if grouped_relics.is_empty() {
            response_text.push_str("  No relics found for this part.\n\n");
            continue;
        }

        for (relic_name, entries) in grouped_relics {
            response_text.push_str(&format!("  {} -> {}\n", relic_name, format_state_chances(entries)));
        }

        response_text.push('\n');
    }

    response_text
}

async fn say_chunked(ctx: Context<'_>, content: &str) -> Result<(), Error> {
    const MAX_CHARS: usize = 1_900;

    if content.chars().count() <= MAX_CHARS {
        ctx.say(content).await?;
        return Ok(());
    }

    let mut current = String::new();
    for line in content.lines() {
        let candidate_len = current.chars().count() + line.chars().count() + 1;
        if candidate_len > MAX_CHARS && !current.is_empty() {
            ctx.say(current.clone()).await?;
            current.clear();
        }

        current.push_str(line);
        current.push('\n');
    }

    if !current.is_empty() {
        ctx.say(current).await?;
    }

    Ok(())
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

        response_text.push_str(&format!("  {}\n", format_state_chances(entries)));

        response_text.push('\n');
    }

    response_text
}

async fn handle_set_query(ctx: Context<'_>, relics: &RelicResponse, set_name: &str) -> Result<(), Error> {
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
    for part in set_parts {
        let grouped_relics = group_sorted_relics(relics, &normalize_text(&part));
        part_groups.push((part, grouped_relics));
    }

    let response_text = format_set_response(set_name, &part_groups);
    say_chunked(ctx, &response_text).await
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

    if let Some(set_name) = parse_prime_set_query(&item) {
        handle_set_query(ctx, &relics, &set_name).await?;
        return Ok(());
    }

    let search_term = normalize_text(&item);
    let mut found_relics = collect_matching_relics(&relics, &search_term);

    if found_relics.is_empty() {
        ctx.say(format!("No relics found containing {}\n\nMake sure to include 'Prime' in your search (e.g., 'Ash Prime Systems')", item))
            .await?;
        return Ok(());
    }

    sort_relics(&mut found_relics);
    let grouped_relics = group_relics(found_relics);
    let response_text = format_relic_response(&item, &grouped_relics);

    say_chunked(ctx, &response_text).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_prime_set_name() {
        assert_eq!(extract_prime_set_name("Ash Prime Systems"), Some("Ash Prime".to_string()));
        assert_eq!(extract_prime_set_name("Ash Prime"), Some("Ash Prime".to_string()));
        assert_eq!(extract_prime_set_name("Braton Prime Barrel"), Some("Braton Prime".to_string()));
        assert_eq!(extract_prime_set_name("Forma BP"), None);
        assert_eq!(extract_prime_set_name("Prime Part"), None);
    }

    #[test]
    fn test_parse_prime_set_query() {
        assert_eq!(parse_prime_set_query("Ash Prime Set"), Some("Ash Prime".to_string()));
        assert_eq!(parse_prime_set_query("Ash Prime"), Some("Ash Prime".to_string()));
        assert_eq!(parse_prime_set_query("Ash"), None);
        assert_eq!(parse_prime_set_query("Prime Set"), None);
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
    fn test_state_order() {
        assert_eq!(state_order("Intact"), 0);
        assert_eq!(state_order("Radiant"), 3);
        assert_eq!(state_order("Unknown"), 4);
    }

    #[test]
    fn test_format_state_chances_orders_by_known_state_priority() {
        let entries = vec![
            FoundRelic {
                tier: "Lith".to_string(),
                relic_name: "Lith A1".to_string(),
                state: "Radiant".to_string(),
                chance: 10.0,
            },
            FoundRelic {
                tier: "Lith".to_string(),
                relic_name: "Lith A1".to_string(),
                state: "Intact".to_string(),
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
                    tier: "Lith".to_string(),
                    relic_name: Some("A1".to_string()),
                    state: "Intact".to_string(),
                    rewards: vec![RelicReward {
                        item_name: "Ash Prime Systems".to_string(),
                        chance: 2.0,
                    }],
                },
                Relic {
                    _id: "2".to_string(),
                    tier: "Lith".to_string(),
                    relic_name: Some("A1".to_string()),
                    state: "Radiant".to_string(),
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
}
