use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Faction {
    pub symbol: String,
    pub name: String,
    pub description: String,
    pub headquarters: String,
    pub traits: Vec<Trait>,
    pub is_recruiting: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trait {
    pub symbol: String,
    pub name: String,
    pub description: String,
}
