use crate::models::{ShipBehaviour, ShipConfig, Waypoint};

pub fn ship_config(waypoints: &Vec<Waypoint>) -> Vec<ShipConfig> {
    let mut ships = vec![];
    ships.push(ShipConfig {
        id: "cmd".to_string(),
        ship_model: "SHIP_COMMAND_FRIGATE".to_string(),
        behaviour: ShipBehaviour::Logistics,
        era: 1,
    });
    for w in waypoints.iter().filter(|w| w.is_market()) {
        let era = if w.is_shipyard() { 2 } else { 3 };
        ships.push(ShipConfig {
            id: format!("probe/{}", w.symbol),
            ship_model: "SHIP_PROBE".to_string(),
            behaviour: ShipBehaviour::FixedProbe(w.symbol.clone()),
            era,
        });
    }

    // @@ disable haulers + siphoning
    // const NUM_LHAULERS: usize = 2;
    // for i in 1..=NUM_LHAULERS {
    //     ships.push(ShipConfig {
    //         id: format!("logistics_lhauler/{}", i),
    //         ship_model: "SHIP_LIGHT_HAULER".to_string(),
    //         behaviour: ShipBehaviour::Logistics,
    //         era: 4,
    //     });
    // }

    // // Era 5: Siphon drones
    // const NUM_SIPHON_DRONES: usize = 10;
    // const NUM_SIPHON_SHUTTLES: usize = 2;
    // for i in 1..=NUM_SIPHON_DRONES {
    //     ships.push(ShipConfig {
    //         id: format!("siphon_drone/{}", i),
    //         ship_model: "SHIP_SIPHON_DRONE".to_string(),
    //         behaviour: ShipBehaviour::SiphonDrone,
    //         era: 5,
    //     });
    // }
    // for i in 1..=NUM_SIPHON_SHUTTLES {
    //     ships.push(ShipConfig {
    //         id: format!("siphon_shuttle/{}", i),
    //         ship_model: "SHIP_LIGHT_HAULER".to_string(),
    //         behaviour: ShipBehaviour::SiphonShuttle,
    //         era: 5,
    //     });
    // }

    const NUM_SURVEYORS: usize = 1;
    const NUM_MINING_DRONES: usize = 4;
    const NUM_MINING_SHUTTLES: usize = 2;
    for i in 1..=NUM_SURVEYORS {
        ships.push(ShipConfig {
            id: format!("surveyor/{}", i),
            ship_model: "SHIP_SURVEYOR".to_string(),
            behaviour: ShipBehaviour::MiningSurveyor,
            era: 6,
        });
    }
    for i in 1..=NUM_MINING_DRONES {
        ships.push(ShipConfig {
            id: format!("mining_drone/{}", i),
            ship_model: "SHIP_MINING_DRONE".to_string(),
            behaviour: ShipBehaviour::MiningDrone,
            era: 6,
        });
    }
    for i in 1..=NUM_MINING_SHUTTLES {
        ships.push(ShipConfig {
            id: format!("mining_shuttle/{}", i),
            ship_model: "SHIP_LIGHT_HAULER".to_string(),
            behaviour: ShipBehaviour::MiningShuttle,
            era: 6,
        });
    }

    ships.sort_by_key(|c| c.era);
    ships
}
