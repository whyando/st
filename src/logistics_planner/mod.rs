pub mod plan;
use crate::models::WaypointSymbol;
use serde::{Deserialize, Serialize};

// An action that can be taken at a waypoint
#[derive(Debug, Clone, Ord, PartialOrd, Eq, PartialEq, Serialize, Deserialize, Hash)]
pub enum Action {
    // load cargo
    BuyGoods(String, i64),
    // unload cargo
    SellGoods(String, i64),
    DeliverContract(String, i64),
    DeliverConstruction(String, i64),
    // actions that don't involve cargo
    RefreshMarket,
    RefreshShipyard,
    TryBuyShips,
    GetContract,
}

impl Action {
    pub fn net_cargo(&self) -> Option<(String, i64)> {
        match self {
            Action::BuyGoods(good, qty) => Some((good.clone(), *qty)),
            Action::SellGoods(good, qty) => Some((good.clone(), -qty)),
            Action::DeliverContract(good, qty) => Some((good.clone(), -qty)),
            Action::DeliverConstruction(good, qty) => Some((good.clone(), -qty)),
            Action::RefreshMarket => None,
            Action::RefreshShipyard => None,
            Action::TryBuyShips => None,
            Action::GetContract => None,
        }
    }
}

#[derive(Debug, Clone, Ord, PartialOrd, Eq, PartialEq, Serialize, Deserialize, Hash)]
pub struct Task {
    pub id: String,
    pub actions: TaskActions,
    pub value: i64,
}

#[derive(Debug, Clone, Ord, PartialOrd, Eq, PartialEq, Serialize, Deserialize, Hash)]
pub enum TaskActions {
    VisitLocation {
        waypoint: WaypointSymbol,
        action: Action,
    },
    TransportCargo {
        src: WaypointSymbol,
        dest: WaypointSymbol,
        src_action: Action,
        dest_action: Action,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogisticShip {
    pub symbol: String,
    pub capacity: i64,
    pub speed: i64,
    pub start_waypoint: WaypointSymbol,
}

#[derive(Debug, Clone)]
pub struct PlannerConstraints {
    pub plan_length: chrono::Duration,
    pub max_compute_time: chrono::Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledAction {
    pub waypoint: WaypointSymbol,
    pub action: Action,
    pub timestamp: i64,
    pub task_completed: Option<Task>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShipSchedule {
    pub ship: LogisticShip,
    pub actions: Vec<ScheduledAction>,
}
