use crate::api_client::api_models;
use crate::models::{SystemSymbol, WaypointSymbol};
use serde::{Deserialize, Serialize};

///
/// Simplified: output from systems.json, and for uncharted systems
/// Detailed: output from /systems/:system_symbol}/waypoints
///
/// Main difference is traits.
///
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Waypoint {
    Simplified(api_models::WaypointSimplified),
    Detailed(api_models::WaypointDetailed),
}

impl Waypoint {
    pub fn symbol(&self) -> &WaypointSymbol {
        match self {
            Waypoint::Simplified(w) => &w.symbol,
            Waypoint::Detailed(w) => &w.symbol,
        }
    }

    pub fn waypoint_type(&self) -> &str {
        match self {
            Waypoint::Simplified(w) => &w.waypoint_type,
            Waypoint::Detailed(w) => &w.waypoint_type,
        }
    }

    pub fn x(&self) -> i64 {
        match self {
            Waypoint::Simplified(w) => w.x,
            Waypoint::Detailed(w) => w.x,
        }
    }

    pub fn y(&self) -> i64 {
        match self {
            Waypoint::Simplified(w) => w.y,
            Waypoint::Detailed(w) => w.y,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct System {
    pub symbol: SystemSymbol,
    #[serde(rename = "type")]
    pub system_type: String,
    pub x: i64,
    pub y: i64,
    pub waypoints: Vec<Waypoint>,
}

impl System {
    pub fn is_starter_system(&self) -> bool {
        self.waypoints
            .iter()
            .any(|w| w.waypoint_type() == "ENGINEERED_ASTEROID")
    }
}
