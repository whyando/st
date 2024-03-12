use crate::models::*;
use std::collections::BTreeMap;

fn market_waypoints(waypoints: &Vec<Waypoint>, range: Option<i32>) -> Vec<WaypointSymbol> {
    waypoints
        .iter()
        .filter(|w| w.is_market())
        // we exclude fuel stations and engineered asteroids because they only trade fuel,
        // so they don't have trading opportunity
        .filter(|w| w.waypoint_type != "FUEL_STATION")
        .filter(|w| w.waypoint_type != "ENGINEERED_ASTEROID")
        .filter(|w| {
            if let Some(range) = range {
                let dist_from_origin = (w.x * w.x + w.y * w.y) as i32;
                dist_from_origin <= range
            } else {
                true
            }
        })
        .map(|w| w.symbol.clone())
        .collect()
}

pub fn ship_config(
    waypoints: &Vec<Waypoint>,
    _markets: &Vec<MarketRemoteView>,
    _shipyards: &Vec<ShipyardRemoteView>,
) -> Vec<ShipConfig> {
    let mut ships = vec![];

    let inner_market_waypoints = market_waypoints(waypoints, Some(200));
    let all_market_waypoints = market_waypoints(waypoints, None);

    // Command frigate trades on logistics planner, but is restricted to 200 units from origin
    ships.push((
        (1.0, 0.0),
        ShipConfig {
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
        },
    ));

    // Send probes to all inner markets with shipyards getting priority
    // probes rotate through all waypoints at a location
    let mut probe_locations = BTreeMap::new();
    for w in waypoints
        .iter()
        .filter(|w| inner_market_waypoints.contains(&w.symbol))
    {
        let loc = if w.is_shipyard() {
            w.symbol.to_string()
        } else {
            format!("({},{})", w.x, w.y)
        };
        let e = probe_locations.entry(loc).or_insert_with(|| {
            let dist = ((w.x * w.x + w.y * w.y) as f64).sqrt() as i64;
            (vec![], w.is_shipyard(), dist)
        });
        e.0.push(w.symbol.clone());
    }
    for (loc, (waypoints, has_shipyard, dist)) in probe_locations {
        let config = ProbeScriptConfig { waypoints };
        let order = -10000.0 * (has_shipyard as i64 as f64) + (dist as f64);
        ships.push((
            (2.0, order),
            ShipConfig {
                id: format!("probe/{}", loc),
                ship_model: "SHIP_PROBE".to_string(),
                behaviour: ShipBehaviour::Probe(config),
                purchase_criteria: PurchaseCriteria {
                    allow_logistic_task: true,
                    require_cheapest: false,
                    ..PurchaseCriteria::default()
                },
            },
        ));
    }

    // Mining operation
    const NUM_SURVEYORS: i64 = 1;
    const NUM_MINING_DRONES: i64 = 8;
    const NUM_MINING_SHUTTLES: i64 = 2;
    for i in 0..NUM_SURVEYORS {
        ships.push((
            (3.0, (i as f64) / (NUM_SURVEYORS as f64)),
            ShipConfig {
                id: format!("surveyor/{}", i),
                ship_model: "SHIP_SURVEYOR".to_string(),
                purchase_criteria: PurchaseCriteria::default(),
                behaviour: ShipBehaviour::MiningSurveyor,
            },
        ));
    }
    for i in 0..NUM_MINING_DRONES {
        ships.push((
            (3.0, (i as f64) / (NUM_MINING_DRONES as f64)),
            ShipConfig {
                id: format!("mining_drone/{}", i),
                ship_model: "SHIP_MINING_DRONE".to_string(),
                purchase_criteria: PurchaseCriteria::default(),
                behaviour: ShipBehaviour::MiningDrone,
            },
        ));
    }
    for i in 0..NUM_MINING_SHUTTLES {
        ships.push((
            (3.0, (i as f64) / (NUM_MINING_SHUTTLES as f64)),
            ShipConfig {
                id: format!("mining_shuttle/{}", i),
                ship_model: "SHIP_LIGHT_HAULER".to_string(),
                purchase_criteria: PurchaseCriteria::default(),
                behaviour: ShipBehaviour::MiningShuttle,
            },
        ));
    }

    // Dedicated jump gate construction hauler
    ships.push((
        (4.0, 0.0),
        ShipConfig {
            id: "jump_gate_hauler".to_string(),
            ship_model: "SHIP_LIGHT_HAULER".to_string(),
            purchase_criteria: PurchaseCriteria::default(),
            behaviour: ShipBehaviour::ConstructionHauler,
        },
    ));

    // !! time gap / credit gap to make sure we start the construction asap

    // Add probes for the remaining markets - should we convert the old ones to static probes everywhere??
    for w in waypoints
        .iter()
        .filter(|w| all_market_waypoints.contains(&w.symbol))
        .filter(|w| !inner_market_waypoints.contains(&w.symbol))
    {
        let config = ProbeScriptConfig {
            waypoints: vec![w.symbol.clone()],
        };
        ships.push((
            (5.0, 0.0),
            ShipConfig {
                id: format!("probe/{}", w.symbol),
                ship_model: "SHIP_PROBE".to_string(),
                behaviour: ShipBehaviour::Probe(config),
                purchase_criteria: PurchaseCriteria::default(),
            },
        ));
    }

    // Add 3 logistics haulers - not using planner
    const NUM_LHAULERS: i64 = 3;
    for i in 0..NUM_LHAULERS {
        ships.push((
            (6.0, (i as f64) / (NUM_LHAULERS as f64)),
            ShipConfig {
                id: format!("logistics_lhauler/{}", i),
                ship_model: "SHIP_LIGHT_HAULER".to_string(),
                purchase_criteria: PurchaseCriteria::default(),
                behaviour: ShipBehaviour::Logistics(LogisticsScriptConfig {
                    use_planner: false,
                    waypoint_allowlist: None,
                    allow_shipbuying: false,
                    allow_market_refresh: false,
                    allow_construction: false,
                }),
            },
        ));
    }

    // Siphon drones + haulers
    const NUM_SIPHON_DRONES: usize = 8;
    const NUM_SIPHON_SHUTTLES: usize = 1;
    for i in 0..NUM_SIPHON_DRONES {
        ships.push((
            (7.0, (i as f64) / (NUM_SIPHON_DRONES as f64)),
            ShipConfig {
                id: format!("siphon_drone/{}", i),
                ship_model: "SHIP_SIPHON_DRONE".to_string(),
                purchase_criteria: PurchaseCriteria::default(),
                behaviour: ShipBehaviour::SiphonDrone,
            },
        ));
    }
    for i in 0..NUM_SIPHON_SHUTTLES {
        ships.push((
            (7.0, (i as f64) / (NUM_SIPHON_SHUTTLES as f64)),
            ShipConfig {
                id: format!("siphon_shuttle/{}", i),
                ship_model: "SHIP_LIGHT_HAULER".to_string(),
                purchase_criteria: PurchaseCriteria::default(),
                behaviour: ShipBehaviour::SiphonShuttle,
            },
        ));
    }

    ships.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    ships.into_iter().map(|(_, c)| c).collect()
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
