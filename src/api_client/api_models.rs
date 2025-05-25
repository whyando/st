use super::{SystemSymbol, WaypointSymbol};
use crate::models::SymbolNameDescr;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct System {
    pub symbol: SystemSymbol,
    #[serde(rename = "type")]
    pub system_type: String,
    pub x: i64,
    pub y: i64,
    pub waypoints: Vec<WaypointSimplified>,
    // pub factions: Vec<Symbol>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WaypointSimplified {
    pub symbol: WaypointSymbol,
    #[serde(rename = "type")]
    pub waypoint_type: String,
    pub x: i64,
    pub y: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WaypointDetailed {
    pub system_symbol: SystemSymbol,
    pub symbol: WaypointSymbol,
    #[serde(rename = "type")]
    pub waypoint_type: String,
    pub x: i64,
    pub y: i64,
    pub traits: Vec<SymbolNameDescr>,
    // pub faction: Option<Symbol>,
    pub is_under_construction: bool,
    // orbitals
    // modifiers
    // chart
}

impl WaypointDetailed {
    pub fn is_uncharted(&self) -> bool {
        self.traits.iter().any(|t| t.symbol == "UNCHARTED")
    }
    pub fn is_market(&self) -> bool {
        self.traits.iter().any(|t| t.symbol == "MARKETPLACE")
    }
    pub fn is_shipyard(&self) -> bool {
        self.traits.iter().any(|t| t.symbol == "SHIPYARD")
    }
    pub fn is_jump_gate(&self) -> bool {
        self.waypoint_type == "JUMP_GATE"
    }
    pub fn is_gas_giant(&self) -> bool {
        self.waypoint_type == "GAS_GIANT"
    }
    pub fn is_engineered_asteroid(&self) -> bool {
        self.waypoint_type == "ENGINEERED_ASTEROID"
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        api_client::WaypointSymbol,
        models::{Data, PaginatedList},
    };

    #[test]
    fn test_get_system() {
        // get system response
        let system_json = r#"{"data":{"symbol":"X1-HN18","sectorSymbol":"X1","type":"ORANGE_STAR","x":-4792,"y":-5050,"waypoints":[{"symbol":"X1-HN18-ZX1B","type":"GAS_GIANT","x":16,"y":3,"orbitals":[]},{"symbol":"X1-HN18-DD4X","type":"ASTEROID","x":116,"y":-713,"orbitals":[]},{"symbol":"X1-HN18-EB5E","type":"ASTEROID","x":694,"y":249,"orbitals":[]},{"symbol":"X1-HN18-ED6Z","type":"ASTEROID","x":-464,"y":573,"orbitals":[]},{"symbol":"X1-HN18-XC8X","type":"ASTEROID","x":-371,"y":619,"orbitals":[]},{"symbol":"X1-HN18-FB9Z","type":"ASTEROID","x":-228,"y":724,"orbitals":[]},{"symbol":"X1-HN18-FB2D","type":"ASTEROID","x":-621,"y":-423,"orbitals":[]},{"symbol":"X1-HN18-CD7D","type":"ASTEROID","x":-203,"y":733,"orbitals":[]},{"symbol":"X1-HN18-DB3Z","type":"ASTEROID","x":-28,"y":-779,"orbitals":[]},{"symbol":"X1-HN18-F10F","type":"ASTEROID","x":-510,"y":-532,"orbitals":[]}],"factions":[]}}"#;
        let system: Data<System> = serde_json::from_str(system_json).unwrap();
        assert_eq!(system.data.symbol, SystemSymbol::new("X1-HN18"));
    }

    #[test]
    fn test_get_waypoints() {
        // get waypoints response
        let waypoint_json = r#"{"data":[{"systemSymbol":"X1-HN18","symbol":"X1-HN18-ZX1B","type":"GAS_GIANT","x":16,"y":3,"orbitals":[],"traits":[{"symbol":"UNCHARTED","name":"Uncharted","description":"An unexplored region of space, full of potential discoveries and hidden dangers."}],"modifiers":[],"isUnderConstruction":false},{"systemSymbol":"X1-HN18","symbol":"X1-HN18-DD4X","type":"ASTEROID","x":116,"y":-713,"orbitals":[],"traits":[{"symbol":"UNCHARTED","name":"Uncharted","description":"An unexplored region of space, full of potential discoveries and hidden dangers."}],"modifiers":[],"isUnderConstruction":false},{"systemSymbol":"X1-HN18","symbol":"X1-HN18-EB5E","type":"ASTEROID","x":694,"y":249,"orbitals":[],"traits":[{"symbol":"UNCHARTED","name":"Uncharted","description":"An unexplored region of space, full of potential discoveries and hidden dangers."}],"modifiers":[],"isUnderConstruction":false},{"systemSymbol":"X1-HN18","symbol":"X1-HN18-ED6Z","type":"ASTEROID","x":-464,"y":573,"orbitals":[],"traits":[{"symbol":"UNCHARTED","name":"Uncharted","description":"An unexplored region of space, full of potential discoveries and hidden dangers."}],"modifiers":[],"isUnderConstruction":false},{"systemSymbol":"X1-HN18","symbol":"X1-HN18-XC8X","type":"ASTEROID","x":-371,"y":619,"orbitals":[],"traits":[{"symbol":"UNCHARTED","name":"Uncharted","description":"An unexplored region of space, full of potential discoveries and hidden dangers."}],"modifiers":[],"isUnderConstruction":false},{"systemSymbol":"X1-HN18","symbol":"X1-HN18-FB9Z","type":"ASTEROID","x":-228,"y":724,"orbitals":[],"traits":[{"symbol":"UNCHARTED","name":"Uncharted","description":"An unexplored region of space, full of potential discoveries and hidden dangers."}],"modifiers":[],"isUnderConstruction":false},{"systemSymbol":"X1-HN18","symbol":"X1-HN18-FB2D","type":"ASTEROID","x":-621,"y":-423,"orbitals":[],"traits":[{"symbol":"UNCHARTED","name":"Uncharted","description":"An unexplored region of space, full of potential discoveries and hidden dangers."}],"modifiers":[],"isUnderConstruction":false},{"systemSymbol":"X1-HN18","symbol":"X1-HN18-CD7D","type":"ASTEROID","x":-203,"y":733,"orbitals":[],"traits":[{"symbol":"UNCHARTED","name":"Uncharted","description":"An unexplored region of space, full of potential discoveries and hidden dangers."}],"modifiers":[],"isUnderConstruction":false},{"systemSymbol":"X1-HN18","symbol":"X1-HN18-DB3Z","type":"ASTEROID","x":-28,"y":-779,"orbitals":[],"traits":[{"symbol":"UNCHARTED","name":"Uncharted","description":"An unexplored region of space, full of potential discoveries and hidden dangers."}],"modifiers":[],"isUnderConstruction":false},{"systemSymbol":"X1-HN18","symbol":"X1-HN18-F10F","type":"ASTEROID","x":-510,"y":-532,"orbitals":[],"traits":[{"symbol":"UNCHARTED","name":"Uncharted","description":"An unexplored region of space, full of potential discoveries and hidden dangers."}],"modifiers":[],"isUnderConstruction":false}],"meta":{"total":10,"page":1,"limit":10}}"#;
        let waypoints: PaginatedList<WaypointDetailed> =
            serde_json::from_str(waypoint_json).unwrap();
        assert_eq!(
            waypoints.data[0].symbol,
            WaypointSymbol::new("X1-HN18-ZX1B")
        );
        assert_eq!(waypoints.data.len(), 10);
    }
}
