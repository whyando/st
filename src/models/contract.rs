use crate::models::WaypointSymbol;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Hash)]
#[serde(rename_all = "camelCase")]
pub struct Contract {
    pub id: String,
    pub faction_symbol: String,
    #[serde(rename = "type")]
    pub contract_type: String,
    pub terms: Terms,
    pub accepted: bool,
    pub fulfilled: bool,
    pub expiration: DateTime<Utc>,
    pub deadline_to_accept: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Hash)]
pub struct Terms {
    pub deadline: String,
    pub payment: Payment,
    pub deliver: Vec<Deliver>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Hash)]
#[serde(rename_all = "camelCase")]
pub struct Payment {
    pub on_fulfilled: i64,
    pub on_accepted: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Hash)]
#[serde(rename_all = "camelCase")]
pub struct Deliver {
    pub trade_symbol: String,
    pub destination_symbol: WaypointSymbol,
    pub units_required: i64,
    pub units_fulfilled: i64,
}
