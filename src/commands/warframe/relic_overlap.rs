use super::relic_farming::{for_each_mission_reward, relic_name_from_item};
use super::types::MissionApiResponse;
use std::collections::{BTreeMap, HashSet};

#[derive(Debug, Clone, PartialEq)]
pub struct OverlapDrop {
    pub relic_name: String,
    pub rotation: String,
    pub chance: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OverlappingMission {
    pub mission_label: String,
    pub game_mode: String,
    pub covered_relics: Vec<String>,
    pub drops_by_rotation: BTreeMap<String, Vec<OverlapDrop>>,
    pub total_chance: f64,
}

pub fn game_mode_rank(mode: &str) -> usize {
    match mode {
        "Disruption" => 0,
        "Capture" => 1,
        "Sabotage" | "Spy" | "Rescue" => 2,
        "Interception" => 3,
        "Defense" | "Survival" | "Excavation" => 4,
        _ => 5,
    }
}

pub fn find_overlapping_missions(
    api_response: &MissionApiResponse,
    target_relics: &HashSet<String>,
) -> Vec<OverlappingMission> {
    if target_relics.is_empty() {
        return Vec::new();
    }

    let mut missions = Vec::new();

    for (planet, p_missions) in &api_response.mission_rewards {
        for (mission_name, mission_data) in p_missions {
            if mission_data.is_event.unwrap_or(false) {
                continue;
            }

            let mut drops_by_rotation: BTreeMap<String, Vec<OverlapDrop>> = BTreeMap::new();
            let mut covered_set = HashSet::new();
            let mut total_chance = 0.0;

            for_each_mission_reward(mission_data, |rotation, reward| {
                if let Some(relic) = relic_name_from_item(&reward.item_name)
                    && target_relics.contains(&relic)
                {
                    covered_set.insert(relic.clone());
                    total_chance += reward.chance;
                    drops_by_rotation
                        .entry(rotation.to_string())
                        .or_default()
                        .push(OverlapDrop {
                            relic_name: relic,
                            rotation: rotation.to_string(),
                            chance: reward.chance,
                        });
                }
            });

            if !covered_set.is_empty() {
                let mut covered_relics: Vec<String> = covered_set.into_iter().collect();
                covered_relics.sort();

                for rot_drops in drops_by_rotation.values_mut() {
                    rot_drops.sort_by(|a, b| {
                        b.chance
                            .partial_cmp(&a.chance)
                            .unwrap_or(std::cmp::Ordering::Equal)
                            .then_with(|| a.relic_name.cmp(&b.relic_name))
                    });
                }

                let mission_label =
                    format!("{}/{} ({})", planet, mission_name, mission_data.game_mode);
                missions.push(OverlappingMission {
                    mission_label,
                    game_mode: mission_data.game_mode.clone(),
                    covered_relics,
                    drops_by_rotation,
                    total_chance,
                });
            }
        }
    }

    // Filter to multi-relic overlaps if any exist (>= 2 relics)
    let has_multi_relic = missions.iter().any(|m| m.covered_relics.len() >= 2);
    if has_multi_relic {
        missions.retain(|m| m.covered_relics.len() >= 2);
    }

    // Sort by: covered count desc, game mode rank asc, total chance desc
    missions.sort_by(|a, b| {
        b.covered_relics
            .len()
            .cmp(&a.covered_relics.len())
            .then_with(|| game_mode_rank(&a.game_mode).cmp(&game_mode_rank(&b.game_mode)))
            .then_with(|| {
                b.total_chance
                    .partial_cmp(&a.total_chance)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    });

    missions
}

pub fn format_set_overlapping_missions(
    api_response: &MissionApiResponse,
    target_relics: &HashSet<String>,
    limit: usize,
) -> String {
    let missions = find_overlapping_missions(api_response, target_relics);

    if missions.is_empty() {
        return "All relics for this Prime set are currently vaulted (no active mission drop spots).\n"
            .to_string();
    }

    let mut out = String::new();
    let take_count = limit.min(missions.len());

    for mission in &missions[..take_count] {
        let count = mission.covered_relics.len();
        let relic_label = if count == 1 { "relic" } else { "relics" };
        out.push_str(&format!(
            "**{}** — **{} {} covered**\n",
            mission.mission_label, count, relic_label
        ));

        for (rotation, drops) in &mission.drops_by_rotation {
            let drop_strs: Vec<String> = drops
                .iter()
                .map(|d| format!("{} ({:.1}%)", d.relic_name, d.chance))
                .collect();
            out.push_str(&format!("└ Rot {}: {}\n", rotation, drop_strs.join(", ")));
        }
        out.push('\n');
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::warframe::types::{MissionData, MissionReward, MissionRewards};
    use std::collections::HashMap;

    fn sample_reward(item_name: &str, chance: f64) -> MissionReward {
        MissionReward {
            item_name: item_name.to_string(),
            rarity: "Common".to_string(),
            chance,
        }
    }

    #[test]
    fn test_find_overlapping_missions_ranks_and_filters() {
        let mut mission_rewards = HashMap::new();

        // Node 1: Lua Apollo (Disruption) with Lith A1 on A, Neo N1 on B, Axi A1 on C (3 relics)
        let apollo = MissionData {
            game_mode: "Disruption".to_string(),
            is_event: Some(false),
            rewards: MissionRewards::Rotations(HashMap::from([
                ("A".to_string(), vec![sample_reward("Lith A1 Relic", 14.0)]),
                ("B".to_string(), vec![sample_reward("Neo N1 Relic", 14.0)]),
                ("C".to_string(), vec![sample_reward("Axi A1 Relic", 12.0)]),
            ])),
        };

        // Node 2: Void Ukko (Capture) with Lith A1 and Neo N1 (2 relics)
        let ukko = MissionData {
            game_mode: "Capture".to_string(),
            is_event: Some(false),
            rewards: MissionRewards::List(vec![
                sample_reward("Lith A1 Relic", 6.0),
                sample_reward("Neo N1 Relic", 6.0),
            ]),
        };

        // Node 3: Earth E Prime (Capture) with only Lith A1 (1 relic)
        let e_prime = MissionData {
            game_mode: "Capture".to_string(),
            is_event: Some(false),
            rewards: MissionRewards::List(vec![sample_reward("Lith A1 Relic", 10.0)]),
        };

        mission_rewards.insert(
            "Lua".to_string(),
            HashMap::from([("Apollo".to_string(), apollo)]),
        );
        mission_rewards.insert(
            "Void".to_string(),
            HashMap::from([("Ukko".to_string(), ukko)]),
        );
        mission_rewards.insert(
            "Earth".to_string(),
            HashMap::from([("E Prime".to_string(), e_prime)]),
        );

        let api_response = MissionApiResponse { mission_rewards };

        let targets = HashSet::from([
            "Lith A1".to_string(),
            "Neo N1".to_string(),
            "Axi A1".to_string(),
        ]);

        let overlaps = find_overlapping_missions(&api_response, &targets);

        // Earth E Prime should be filtered out because multi-relic nodes exist
        assert_eq!(overlaps.len(), 2);
        // Apollo covers 3 relics, should be first
        assert_eq!(overlaps[0].mission_label, "Lua/Apollo (Disruption)");
        assert_eq!(overlaps[0].covered_relics.len(), 3);
        // Ukko covers 2 relics, should be second
        assert_eq!(overlaps[1].mission_label, "Void/Ukko (Capture)");
        assert_eq!(overlaps[1].covered_relics.len(), 2);

        let formatted = format_set_overlapping_missions(&api_response, &targets, 5);
        assert!(formatted.contains("Lua/Apollo (Disruption)"));
        assert!(formatted.contains("3 relics covered"));
        assert!(formatted.contains("Void/Ukko (Capture)"));
    }

    #[test]
    fn test_format_vaulted_set_when_no_relics_drop() {
        let api_response = MissionApiResponse {
            mission_rewards: HashMap::new(),
        };

        let targets = HashSet::from(["Ash Prime Relic".to_string()]);
        let formatted = format_set_overlapping_missions(&api_response, &targets, 5);
        assert!(formatted.contains("vaulted"));
    }
}
