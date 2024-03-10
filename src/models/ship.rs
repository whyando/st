use crate::models::{SystemSymbol, WaypointSymbol};
use chrono::{DateTime, Utc};
use maplit::hashmap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Ship {
    pub symbol: String,
    pub nav: ShipNav,
    pub crew: ShipCrew,
    pub fuel: ShipFuel,
    pub cooldown: ShipCooldown,
    pub frame: ShipFrame,
    pub reactor: ShipReactor,
    pub engine: ShipEngine,
    pub modules: Vec<ShipModule>,
    pub mounts: Vec<ShipMount>,
    pub registration: ShipRegistration,
    pub cargo: ShipCargo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShipNav {
    pub system_symbol: SystemSymbol,
    pub waypoint_symbol: WaypointSymbol,
    pub route: ShipNavRoute,
    pub status: ShipNavStatus,
    pub flight_mode: ShipFlightMode,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ShipFlightMode {
    #[serde(rename = "CRUISE")]
    Cruise,
    #[serde(rename = "BURN")]
    Burn,
    #[serde(rename = "DRIFT")]
    Drift,
    #[serde(rename = "STEALTH")]
    Stealth,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ShipNavStatus {
    #[serde(rename = "DOCKED")]
    Docked,
    #[serde(rename = "IN_TRANSIT")]
    InTransit,
    #[serde(rename = "IN_ORBIT")]
    InOrbit,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShipNavRoute {
    // pub departure: ShipNavRouteWaypoint // deprecated
    pub origin: ShipNavRouteWaypoint,
    pub destination: ShipNavRouteWaypoint,
    pub arrival: DateTime<Utc>,
    pub departure_time: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShipNavRouteWaypoint {
    pub symbol: WaypointSymbol,
    #[serde(rename = "type")]
    pub waypoint_type: String,
    pub system_symbol: SystemSymbol,
    pub x: i64,
    pub y: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShipCrew {
    pub current: i64,
    pub capacity: i64,
    pub required: i64,
    pub rotation: String,
    pub morale: i64,
    pub wages: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShipFuel {
    pub current: i64,
    pub capacity: i64,
    pub consumed: ShipFuelConsumed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShipFuelConsumed {
    pub amount: i64,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShipCooldown {
    pub ship_symbol: String,
    pub total_seconds: i64,
    pub remaining_seconds: i64,
    pub expiration: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShipFrame {
    pub symbol: String,
    pub name: String,
    pub description: String,
    pub module_slots: i64,
    pub mounting_points: i64,
    pub fuel_capacity: i64,
    pub condition: Option<f64>,
    pub integrity: Option<f64>,
    pub requirements: ShipRequirements,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShipRequirements {
    #[serde(default)]
    pub power: i64,
    #[serde(default)]
    pub crew: i64,
    #[serde(default)]
    pub slots: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShipReactor {
    pub symbol: String,
    pub name: String,
    pub description: String,
    pub condition: Option<f64>,
    pub integrity: Option<f64>,
    pub power_output: i64,
    pub requirements: ShipRequirements,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShipEngine {
    pub symbol: String,
    pub name: String,
    pub description: String,
    pub condition: Option<f64>,
    pub integrity: Option<f64>,
    pub speed: i64,
    pub requirements: ShipRequirements,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShipModule {
    pub symbol: String,
    pub name: String,
    pub description: String,
    pub capacity: Option<i64>,
    pub requirements: ShipRequirements,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShipMount {
    pub symbol: String,
    pub name: String,
    pub description: String,
    pub strength: Option<i64>,
    pub requirements: ShipRequirements,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShipRegistration {
    pub name: String,
    pub faction_symbol: String,
    pub role: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShipCargo {
    pub capacity: i64,
    pub units: i64,
    pub inventory: Vec<ShipCargoItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShipCargoItem {
    pub symbol: String,
    pub units: i64,
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone)]
pub struct ShipModel {
    pub frame: String,
    pub reactor: String,
    pub engine: String,
    pub req_modules: Vec<String>,
    pub req_mounts: Vec<String>,
    pub cargo_capacity: i64,
}

// ship models
lazy_static::lazy_static! {
    pub static ref SHIP_MODELS: HashMap<&'static str, ShipModel> = hashmap!{
        "SHIP_COMMAND_FRIGATE" => ShipModel {
            frame: "FRAME_FRIGATE".to_string(),
            reactor: "REACTOR_FISSION_I".to_string(),
            engine: "ENGINE_ION_DRIVE_II".to_string(),
            req_modules: vec![],
            req_mounts: vec![],
            cargo_capacity: 40,
        },
        "SHIP_PROBE" => ShipModel {
            frame: "FRAME_PROBE".to_string(),
            reactor: "REACTOR_SOLAR_I".to_string(),
            engine: "ENGINE_IMPULSE_DRIVE_I".to_string(),
            req_modules: vec![],
            req_mounts: vec![],
            cargo_capacity: 0,
        },
        "SHIP_LIGHT_SHUTTLE" => ShipModel {
            frame: "FRAME_SHUTTLE".to_string(),
            reactor: "REACTOR_CHEMICAL_I".to_string(),
            engine: "ENGINE_IMPULSE_DRIVE_I".to_string(),
            req_modules: vec![],
            req_mounts: vec![],
            cargo_capacity: 40,
        },
        "SHIP_LIGHT_HAULER" => ShipModel {
            frame: "FRAME_LIGHT_FREIGHTER".to_string(),
            reactor: "REACTOR_CHEMICAL_I".to_string(),
            engine: "ENGINE_ION_DRIVE_I".to_string(),
            req_modules: vec![],
            req_mounts: vec![],
            cargo_capacity: 80,
        },
        "SHIP_MINING_DRONE" => ShipModel {
            frame: "FRAME_DRONE".to_string(),
            reactor: "REACTOR_CHEMICAL_I".to_string(),
            engine: "ENGINE_IMPULSE_DRIVE_I".to_string(),
            req_modules: vec!["MODULE_MINERAL_PROCESSOR_I".to_string()],
            req_mounts: vec!["MOUNT_MINING_LASER_I".to_string()],
            cargo_capacity: 15,
        },
        "SHIP_SURVEYOR" => ShipModel {
            frame: "FRAME_DRONE".to_string(),
            reactor: "REACTOR_CHEMICAL_I".to_string(),
            engine: "ENGINE_IMPULSE_DRIVE_I".to_string(),
            req_modules: vec![],
            req_mounts: vec!["MOUNT_SURVEYOR_I".to_string()],
            cargo_capacity: 0,
        },
        "SHIP_SIPHON_DRONE" => ShipModel {
            frame: "FRAME_DRONE".to_string(),
            reactor: "REACTOR_CHEMICAL_I".to_string(),
            engine: "ENGINE_IMPULSE_DRIVE_I".to_string(),
            req_modules: vec!["MODULE_GAS_PROCESSOR_I".to_string()],
            req_mounts: vec!["MOUNT_GAS_SIPHON_I".to_string()],
            cargo_capacity: 15,
        },
    };
}

impl Ship {
    pub fn model(&self) -> Result<String, String> {
        // find the model in SHIP_MODELS with matching frame, reactor, and engine
        let matching_models = SHIP_MODELS
            .iter()
            .filter(|(_, ship_model)| self.frame.symbol == ship_model.frame)
            .filter(|(_, ship_model)| self.reactor.symbol == ship_model.reactor)
            .filter(|(_, ship_model)| self.engine.symbol == ship_model.engine)
            .filter(|(_, ship_model)| self.cargo.capacity == ship_model.cargo_capacity)
            .filter(|(_, ship_model)| {
                for module in ship_model.req_modules.iter() {
                    if !self.modules.iter().any(|m| m.symbol == *module) {
                        return false;
                    }
                }
                for mount in ship_model.req_mounts.iter() {
                    if !self.mounts.iter().any(|m| m.symbol == *mount) {
                        return false;
                    }
                }
                true
            })
            .collect::<Vec<(&&str, &ShipModel)>>();
        if matching_models.len() == 1 {
            return Ok(matching_models[0].0.to_string());
        }
        Err(format!(
            "{} matching models for ship {} with frame: {}, reactor: {}, engine: {}",
            matching_models.len(),
            self.symbol,
            self.frame.symbol,
            self.reactor.symbol,
            self.engine.symbol
        ))
    }

    pub fn symbol(&self) -> String {
        self.symbol.clone()
    }

    pub fn incr_cargo(&mut self, item: ShipCargoItem) {
        self.cargo.units += item.units;
        let good = self
            .cargo
            .inventory
            .iter_mut()
            .find(|good| good.symbol == item.symbol);
        match good {
            Some(good) => {
                good.units += item.units;
            }
            None => {
                self.cargo.inventory.push(item);
            }
        }
    }
}
