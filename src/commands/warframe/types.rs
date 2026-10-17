use serde::Deserialize;
use std::collections::HashMap;

#[derive(thiserror::Error, Debug)]
pub enum WarframeError {
    #[error("Warframe API request to {url} failed with status {status}")]
    HttpError {
        url: String,
        status: reqwest::StatusCode,
    },
    #[error("Warframe API request network failure: {0}")]
    Network(#[from] reqwest::Error),
    #[error("Failed to deserialize Warframe API response from {url}: {source}")]
    Deserialize {
        url: String,
        #[source]
        source: reqwest::Error,
    },
}

// --- RELIC LOOKUP TYPES ---

#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RelicTier {
    Lith,
    Meso,
    Neo,
    Axi,
    Requiem,
    #[serde(other)]
    Unknown,
}

impl RelicTier {
    pub fn parse(input: &str) -> Option<Self> {
        match input.to_ascii_lowercase().as_str() {
            "lith" => Some(Self::Lith),
            "meso" => Some(Self::Meso),
            "neo" => Some(Self::Neo),
            "axi" => Some(Self::Axi),
            "requiem" => Some(Self::Requiem),
            _ => None,
        }
    }
}

impl std::fmt::Display for RelicTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RelicTier::Lith => write!(f, "Lith"),
            RelicTier::Meso => write!(f, "Meso"),
            RelicTier::Neo => write!(f, "Neo"),
            RelicTier::Axi => write!(f, "Axi"),
            RelicTier::Requiem => write!(f, "Requiem"),
            RelicTier::Unknown => write!(f, "Unknown"),
        }
    }
}

#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RelicState {
    Intact,
    Exceptional,
    Flawless,
    Radiant,
    #[serde(other)]
    Unknown,
}

impl std::fmt::Display for RelicState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RelicState::Intact => write!(f, "Intact"),
            RelicState::Exceptional => write!(f, "Exceptional"),
            RelicState::Flawless => write!(f, "Flawless"),
            RelicState::Radiant => write!(f, "Radiant"),
            RelicState::Unknown => write!(f, "Unknown"),
        }
    }
}

pub const RELIC_STATES: [RelicState; 4] = [
    RelicState::Intact,
    RelicState::Exceptional,
    RelicState::Flawless,
    RelicState::Radiant,
];

#[derive(Deserialize, Debug)]
pub struct RelicResponse {
    pub relics: Vec<Relic>,
}

#[derive(Deserialize, Debug)]
pub struct Relic {
    #[serde(rename = "_id")]
    pub _id: String,
    pub tier: RelicTier,
    #[serde(rename = "relicName", alias = "name", default)]
    pub relic_name: Option<String>,
    pub state: RelicState,
    pub rewards: Vec<RelicReward>,
}

#[derive(Deserialize, Debug)]
pub struct RelicReward {
    #[serde(rename = "itemName")]
    pub item_name: String,
    pub chance: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RelicSearchQuery {
    PrimeSet(String),
    SpecificItem(String),
}

pub fn parse_prime_set_query(input: &str) -> Option<String> {
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

    let prime_index = tokens
        .iter()
        .position(|token| token.eq_ignore_ascii_case("prime"))?;
    if prime_index == 0 || prime_index != tokens.len() - 1 {
        return None;
    }

    Some(tokens[..=prime_index].join(" "))
}

impl RelicSearchQuery {
    pub fn parse(input: &str) -> Self {
        if let Some(set_name) = parse_prime_set_query(input) {
            Self::PrimeSet(set_name)
        } else {
            Self::SpecificItem(input.to_string())
        }
    }
}

// --- RELIC FARMING TYPES ---

#[derive(Deserialize, Debug)]
pub struct MissionApiResponse {
    #[serde(rename = "missionRewards")]
    pub mission_rewards: HashMap<String, HashMap<String, MissionData>>,
}

#[derive(Deserialize, Debug)]
pub struct MissionData {
    #[serde(rename = "gameMode")]
    pub game_mode: String,
    #[serde(rename = "isEvent")]
    pub is_event: Option<bool>,
    pub rewards: MissionRewards,
}

#[derive(Deserialize, Debug)]
#[serde(untagged)]
pub enum MissionRewards {
    Rotations(HashMap<String, Vec<MissionReward>>),
    List(Vec<MissionReward>),
}

#[derive(Deserialize, Debug)]
pub struct MissionReward {
    #[serde(rename = "itemName")]
    pub item_name: String,
    pub rarity: String,
    pub chance: f64,
}

#[derive(Debug, Clone)]
pub struct RotationDrop {
    pub rotation: String,
    pub rarity: String,
    pub chance: f64,
}

#[derive(Debug, Clone)]
pub struct MissionDrops {
    pub mission_label: String,
    pub drops: Vec<RotationDrop>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_relic_tier_parse() {
        assert_eq!(RelicTier::parse("lith"), Some(RelicTier::Lith));
        assert_eq!(RelicTier::parse("MESO"), Some(RelicTier::Meso));
        assert_eq!(RelicTier::parse("Neo"), Some(RelicTier::Neo));
        assert_eq!(RelicTier::parse("axi"), Some(RelicTier::Axi));
        assert_eq!(RelicTier::parse("requiem"), Some(RelicTier::Requiem));
        assert_eq!(RelicTier::parse("invalid"), None);
    }

    #[test]
    fn test_parse_prime_set_query() {
        assert_eq!(
            parse_prime_set_query("Ash Prime Set"),
            Some("Ash Prime".to_string())
        );
        assert_eq!(
            parse_prime_set_query("Ash Prime"),
            Some("Ash Prime".to_string())
        );
        assert_eq!(parse_prime_set_query("Ash"), None);
        assert_eq!(parse_prime_set_query("Prime Set"), None);
    }
}
