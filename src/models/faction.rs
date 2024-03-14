use serde::{Deserialize, Serialize};

use super::SystemSymbol;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Faction {
    pub symbol: String,
    pub name: String,
    pub description: String,
    pub headquarters: SystemSymbol,
    pub traits: Vec<Trait>,
    pub is_recruiting: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trait {
    pub symbol: String,
    pub name: String,
    pub description: String,
}
