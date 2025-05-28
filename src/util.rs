use crate::{api_client::api_models::WaypointDetailed, models::ShipFlightMode};

const BASE_TRAVEL_TIME: f64 = 15.0;
const TRAVEL_TIME: f64 = 25.0;

// Trait for types that have x,y coordinates
pub trait Coord {
    fn x(&self) -> i64;
    fn y(&self) -> i64;
}

impl Coord for WaypointDetailed {
    fn x(&self) -> i64 {
        self.x
    }
    fn y(&self) -> i64 {
        self.y
    }
}

impl PartialEq for WaypointDetailed {
    fn eq(&self, other: &Self) -> bool {
        self.symbol == other.symbol
    }
}

// Generalized distance function for any type implementing Coord
pub fn distance<T: Coord + PartialEq>(a: &T, b: &T) -> i64 {
    if a == b {
        return 0;
    }
    let d2 = (a.x() - b.x()).pow(2) + (a.y() - b.y()).pow(2);
    std::cmp::max(1, (d2 as f64).sqrt().round() as i64)
}

// Fuel cost
// Doesn't apply to probes
pub fn fuel_cost(flight_mode: &ShipFlightMode, distance: i64) -> i64 {
    match flight_mode {
        ShipFlightMode::Burn => distance * 2,
        ShipFlightMode::Cruise => distance,
        ShipFlightMode::Drift => 1,
        ShipFlightMode::Stealth => distance,
    }
}

// Only an estimate because it's increased by poor engine condition
// Doesn't apply to probes
pub fn estimated_travel_duration(flight_mode: &ShipFlightMode, speed: i64, distance: i64) -> i64 {
    let mult = match flight_mode {
        ShipFlightMode::Cruise => 1.0,
        ShipFlightMode::Burn => 0.5,
        ShipFlightMode::Stealth => 2.0,
        ShipFlightMode::Drift => 10.0,
    };
    (BASE_TRAVEL_TIME + (TRAVEL_TIME * distance as f64 / speed as f64) * mult).round() as i64
}
