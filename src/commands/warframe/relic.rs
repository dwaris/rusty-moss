use crate::{Context, Error};
use serde::Deserialize;

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
    rarity: String,
    chance: f64,
}

fn normalize_text(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
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
    item_name: String,
    rarity: String,
    chance: f64,
}

/// Find which relics contain a specific prime item
#[poise::command(slash_command, prefix_command, category = "Warframe")]
pub async fn relic(
    ctx: Context<'_>,
    #[description = "Prime item to search for (e.g., 'Ash Prime Systems')"]
    #[rest]
    item: String,
) -> Result<(), Error> {
    ctx.defer().await?;

    let client = reqwest::Client::new();
    let response = client
        .get("https://drops.warframestat.us/data/relics.json")
        .send()
        .await?;

    if !response.status().is_success() {
        ctx.say("Failed to fetch relic data from API").await?;
        return Ok(());
    }

    let relics: RelicResponse = response.json().await?;

    // Search for relics containing the item
    let mut found_relics: Vec<FoundRelic> = Vec::new();
    let search_term = normalize_text(&item);

    for relic in relics.relics {
        let Some(relic_name) = relic.relic_name.as_deref() else {
            continue;
        };

        for reward in &relic.rewards {
            if normalize_text(&reward.item_name).contains(&search_term) {
                let relic_name = format!("{} {}", relic.tier, relic_name);
                found_relics.push(FoundRelic {
                    tier: relic.tier.clone(),
                    relic_name,
                    state: relic.state.clone(),
                    item_name: reward.item_name.clone(),
                    rarity: reward.rarity.clone(),
                    chance: reward.chance,
                });
            }
        }
    }

    if found_relics.is_empty() {
        ctx.say(format!("No relics found containing {}\n\nMake sure to include 'Prime' in your search (e.g., 'Ash Prime Systems')", item))
            .await?;
        return Ok(());
    }

    found_relics.sort_by(|a, b| {
        a.tier
            .cmp(&b.tier)
            .then_with(|| a.relic_name.cmp(&b.relic_name))
            .then_with(|| state_order(&a.state).cmp(&state_order(&b.state)))
            .then_with(|| a.item_name.cmp(&b.item_name))
    });

    let mut grouped_relics: Vec<(String, Vec<&FoundRelic>)> = Vec::new();
    for relic in &found_relics {
        if let Some((last_name, entries)) = grouped_relics.last_mut() {
            if *last_name == relic.relic_name {
                entries.push(relic);
                continue;
            }
        }

        grouped_relics.push((relic.relic_name.clone(), vec![relic]));
    }

    // Format response
    let mut response_text = format!("Relics containing {}:\n\n", item);

    for (relic_name, entries) in grouped_relics {
        response_text.push_str(&format!("{}\n", relic_name));

        for entry in entries {
            response_text.push_str(&format!(
                "  {}: {} - {} ({:.2}%)\n",
                entry.state, entry.item_name, entry.rarity, entry.chance
            ));
        }

        response_text.push('\n');
    }

    ctx.say(response_text).await?;
    Ok(())
}
