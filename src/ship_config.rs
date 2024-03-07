use std::collections::BTreeMap;

use crate::models::*;

fn inner_market_waypoints(waypoints: &Vec<Waypoint>) -> Vec<WaypointSymbol> {
    waypoints
        .iter()
        .filter(|w| w.is_market())
        // we exclude fuel stations and engineered asteroids because they only trade fuel,
        // so they don't have trading opportunity
        .filter(|w| w.waypoint_type != "FUEL_STATION")
        .filter(|w| w.waypoint_type != "ENGINEERED_ASTEROID")
        .filter(|w| {
            let dist_from_origin = ((w.x * w.x + w.y * w.y) as f64).sqrt() as i64;
            dist_from_origin <= 200
        })
        .map(|w| w.symbol.clone())
        .collect()
}

pub fn ship_config(waypoints: &Vec<Waypoint>) -> Vec<ShipConfig> {
    let mut ships = vec![];

    let inner_market_waypoints = inner_market_waypoints(waypoints);

    // Command frigate trades on logistics planner, but is restricted to 200 units from origin
    ships.push(ShipConfig {
        id: "cmd".to_string(),
        ship_model: "SHIP_COMMAND_FRIGATE".to_string(),
        purchase_criteria: PurchaseCriteria {
            never_purchase: true,
            ..PurchaseCriteria::default()
        },
        behaviour: ShipBehaviour::Logistics(LogisticsScriptConfig {
            use_planner: true,
            waypoint_allowlist: Some(inner_market_waypoints.clone()),
            allow_shipbuying: true,
            allow_market_refresh: true,
            allow_construction: false,
        }),
        era: 1,
    });

    // Send probes to all inner markets with shipyards getting priority
    // probes rotate through all waypoints at a location
    let mut probe_locations = BTreeMap::new();
    for w in waypoints
        .iter()
        .filter(|w| inner_market_waypoints.contains(&w.symbol))
    {
        let loc = format!("({},{})", w.x, w.y);
        let e = probe_locations.entry(loc).or_insert((vec![], false));
        e.0.push(w.symbol.clone());
        if w.is_shipyard() {
            e.1 = true;
        }
    }
    for (loc, (waypoints, has_shipyard)) in probe_locations {
        let era = if has_shipyard { 2 } else { 3 };
        let config = ProbeScriptConfig { waypoints };
        ships.push(ShipConfig {
            id: format!("probe/{}", loc),
            ship_model: "SHIP_PROBE".to_string(),
            behaviour: ShipBehaviour::Probe(config),
            purchase_criteria: PurchaseCriteria {
                allow_logistic_task: true,
                require_cheapest: false,
                ..PurchaseCriteria::default()
            },
            era,
        });
    }

    // Mining operation
    const NUM_SURVEYORS: i64 = 1;
    const NUM_MINING_DRONES: i64 = 8;
    const NUM_MINING_SHUTTLES: i64 = 2;
    const ERA_WIDTH: i64 = 8;
    for i in 0..NUM_SURVEYORS {
        ships.push(ShipConfig {
            id: format!("surveyor/{}", i),
            ship_model: "SHIP_SURVEYOR".to_string(),
            purchase_criteria: PurchaseCriteria::default(),
            behaviour: ShipBehaviour::MiningSurveyor,
            era: 4 + (ERA_WIDTH * i) / NUM_SURVEYORS,
        });
    }
    for i in 0..NUM_MINING_DRONES {
        ships.push(ShipConfig {
            id: format!("mining_drone/{}", i),
            ship_model: "SHIP_MINING_DRONE".to_string(),
            purchase_criteria: PurchaseCriteria::default(),
            behaviour: ShipBehaviour::MiningDrone,
            era: 4 + (ERA_WIDTH * i) / NUM_MINING_DRONES,
        });
    }
    for i in 0..NUM_MINING_SHUTTLES {
        ships.push(ShipConfig {
            id: format!("mining_shuttle/{}", i),
            ship_model: "SHIP_LIGHT_HAULER".to_string(),
            purchase_criteria: PurchaseCriteria::default(),
            behaviour: ShipBehaviour::MiningShuttle,
            era: 4 + (ERA_WIDTH * i) / NUM_MINING_SHUTTLES,
        });
    }

    // todo: Dedicated hauler for building jump gate

    // todo: (Later)
    // switch to static probes everywhere
    // 3 logistic haulers
    // 5 siphons + 1 hauler

    ships.sort_by_key(|c| c.era);
    ships
}

// pub fn ship_config(waypoints: &Vec<Waypoint>) -> Vec<ShipConfig> {
//     let mut ships = vec![];

//     ships.push(ShipConfig {
//         id: "cmd".to_string(),
//         ship_model: "SHIP_COMMAND_FRIGATE".to_string(),
//         purchase_criteria: PurchaseCriteria {
//             never_purchase: true,
//             ..PurchaseCriteria::default()
//         },
//         behaviour: ShipBehaviour::Logistics,
//         era: 1,
//     });
//     for w in waypoints.iter().filter(|w| w.is_market()) {
//         let era = if w.is_shipyard() { 2 } else { 3 };
//         ships.push(ShipConfig {
//             id: format!("probe/{}", w.symbol),
//             ship_model: "SHIP_PROBE".to_string(),
//             behaviour: ShipBehaviour::FixedProbe(w.symbol.clone()),
//             purchase_criteria: PurchaseCriteria::default(),
//             era,
//         });
//     }

//     const NUM_LHAULERS: usize = 7;
//     for i in 1..=NUM_LHAULERS {
//         ships.push(ShipConfig {
//             id: format!("logistics_lhauler/{}", i),
//             ship_model: "SHIP_LIGHT_HAULER".to_string(),
//             purchase_criteria: PurchaseCriteria::default(),
//             behaviour: ShipBehaviour::Logistics,
//             era: 4,
//         });
//     }

//     // Era 5: Siphon drones
//     const NUM_SIPHON_DRONES: usize = 10;
//     const NUM_SIPHON_SHUTTLES: usize = 4;
//     for i in 1..=NUM_SIPHON_DRONES {
//         ships.push(ShipConfig {
//             id: format!("siphon_drone/{}", i),
//             ship_model: "SHIP_SIPHON_DRONE".to_string(),
//             purchase_criteria: PurchaseCriteria::default(),
//             behaviour: ShipBehaviour::SiphonDrone,
//             era: 5,
//         });
//     }
//     for i in 1..=NUM_SIPHON_SHUTTLES {
//         ships.push(ShipConfig {
//             id: format!("siphon_shuttle/{}", i),
//             ship_model: "SHIP_LIGHT_HAULER".to_string(),
//             purchase_criteria: PurchaseCriteria::default(),
//             behaviour: ShipBehaviour::SiphonShuttle,
//             era: 5,
//         });
//     }

//     // Era 6: Mining surveyors, drones, and shuttles
//     const NUM_SURVEYORS: usize = 1;
//     const NUM_MINING_DRONES: usize = 8;
//     const NUM_MINING_SHUTTLES: usize = 4;
//     for i in 1..=NUM_SURVEYORS {
//         ships.push(ShipConfig {
//             id: format!("surveyor/{}", i),
//             ship_model: "SHIP_SURVEYOR".to_string(),
//             purchase_criteria: PurchaseCriteria::default(),
//             behaviour: ShipBehaviour::MiningSurveyor,
//             era: 6,
//         });
//     }
//     for i in 1..=NUM_MINING_DRONES {
//         ships.push(ShipConfig {
//             id: format!("mining_drone/{}", i),
//             ship_model: "SHIP_MINING_DRONE".to_string(),
//             purchase_criteria: PurchaseCriteria::default(),
//             behaviour: ShipBehaviour::MiningDrone,
//             era: 6,
//         });
//     }
//     for i in 1..=NUM_MINING_SHUTTLES {
//         ships.push(ShipConfig {
//             id: format!("mining_shuttle/{}", i),
//             ship_model: "SHIP_LIGHT_HAULER".to_string(),
//             purchase_criteria: PurchaseCriteria::default(),
//             behaviour: ShipBehaviour::MiningShuttle,
//             era: 6,
//         });
//     }

//     ships.sort_by_key(|c| c.era);
//     ships
// }
