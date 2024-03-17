use crate::models::*;
use std::collections::BTreeMap;

fn market_waypoints(waypoints: &Vec<Waypoint>, range: Option<i64>) -> Vec<WaypointSymbol> {
    waypoints
        .iter()
        .filter(|w| w.is_market())
        // we exclude fuel stations and engineered asteroids because they only trade fuel,
        // so they don't have trading opportunity
        .filter(|w| w.waypoint_type != "FUEL_STATION")
        .filter(|w| w.waypoint_type != "ENGINEERED_ASTEROID")
        .filter(|w| {
            if let Some(range) = range {
                let dist_from_origin = ((w.x * w.x + w.y * w.y) as f64).sqrt() as i64;
                dist_from_origin <= range
            } else {
                true
            }
        })
        .map(|w| w.symbol.clone())
        .collect()
}

pub fn ship_config_starter_system(
    waypoints: &Vec<Waypoint>,
    _markets: &Vec<MarketRemoteView>,
    _shipyards: &Vec<ShipyardRemoteView>,
    use_nonstatic_probes: bool,
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
        let loc = if !w.is_shipyard() && use_nonstatic_probes {
            // use coordinate-grouped probe
            format!("({},{})", w.x, w.y)
        } else {
            w.symbol.to_string()
        };
        let e = probe_locations.entry(loc).or_insert_with(|| {
            let dist = ((w.x * w.x + w.y * w.y) as f64).sqrt() as i64;
            (vec![], w.is_shipyard(), dist)
        });
        e.0.push(w.symbol.clone());
    }
    for (loc, (waypoints, has_shipyard, dist)) in probe_locations {
        let config = ProbeScriptConfig { waypoints };
        if !use_nonstatic_probes {
            assert_eq!(config.waypoints.len(), 1);
        }
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

    // !! insert time gap / credit gap here to make sure we start the construction asap

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

pub fn ship_config_capital_system(
    system_waypoint: &SystemSymbol,
    seed_system: &SystemSymbol,
    waypoints: &Vec<Waypoint>,
    _markets: &Vec<MarketRemoteView>,
    _shipyards: &Vec<ShipyardRemoteView>,
    use_nonstatic_probes: bool,
) -> Vec<ShipConfig> {
    let mut ships = vec![];

    let inner_market_waypoints = market_waypoints(waypoints, Some(200));
    let all_market_waypoints = market_waypoints(waypoints, None);

    // Send probes to all shipyards
    let mut probe_locations = BTreeMap::new();
    for w in waypoints
        .iter()
        .filter(|w| all_market_waypoints.contains(&w.symbol))
    {
        let loc = if !w.is_shipyard() && use_nonstatic_probes {
            // use coordinate-grouped probe
            format!("({},{})", w.x, w.y)
        } else {
            w.symbol.to_string()
        };
        let e = probe_locations.entry(loc).or_insert_with(|| {
            let dist = ((w.x * w.x + w.y * w.y) as f64).sqrt() as i64;
            (vec![], w.is_shipyard(), dist)
        });
        e.0.push(w.symbol.clone());
    }
    for (loc, (waypoints, has_shipyard, dist)) in probe_locations {
        let config = ProbeScriptConfig { waypoints };
        if use_nonstatic_probes {
            assert_eq!(config.waypoints.len(), 1);
        }
        let order = -10000.0 * (has_shipyard as i64 as f64) + (dist as f64);
        let purchase_location = if has_shipyard {
            Some(seed_system.clone())
        } else {
            Some(system_waypoint.clone())
        };
        ships.push((
            (2.0, order),
            ShipConfig {
                id: format!("probe/{}", loc),
                ship_model: "SHIP_PROBE".to_string(),
                behaviour: ShipBehaviour::Probe(config),
                purchase_criteria: PurchaseCriteria {
                    system_symbol: purchase_location,
                    ..PurchaseCriteria::default()
                },
            },
        ));
    }

    // Profit-making haulers: 1x planner, 3x greedy
    ships.push((
        (3.0, 0.0),
        ShipConfig {
            id: format!("logistics_freighter/planned/{}", 1),
            ship_model: "SHIP_REFINING_FREIGHTER".to_string(),
            purchase_criteria: PurchaseCriteria {
                system_symbol: Some(system_waypoint.clone()),
                ..PurchaseCriteria::default()
            },
            behaviour: ShipBehaviour::Logistics(LogisticsScriptConfig {
                use_planner: true,
                waypoint_allowlist: Some(inner_market_waypoints.clone()),
                allow_shipbuying: false,
                allow_market_refresh: false,
                allow_construction: false,
            }),
        },
    ));
    const NUM_GREEDY_FREIGHTERS: i64 = 3;
    for i in 1..=NUM_GREEDY_FREIGHTERS {
        ships.push((
            (3.0, 0.0),
            ShipConfig {
                id: format!("logistics_freighter/greedy/{}", i),
                ship_model: "SHIP_REFINING_FREIGHTER".to_string(),
                purchase_criteria: PurchaseCriteria {
                    system_symbol: Some(system_waypoint.clone()),
                    ..PurchaseCriteria::default()
                },
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
    const NUM_SIPHON_DRONES: usize = 4;
    const NUM_SIPHON_SHUTTLES: usize = 1;
    for i in 0..NUM_SIPHON_DRONES {
        ships.push((
            (7.0, (i as f64) / (NUM_SIPHON_DRONES as f64)),
            ShipConfig {
                id: format!("{}/siphon_drone/{}", system_waypoint, i),
                ship_model: "SHIP_SIPHON_DRONE".to_string(),
                purchase_criteria: PurchaseCriteria {
                    system_symbol: Some(system_waypoint.clone()),
                    ..PurchaseCriteria::default()
                },
                behaviour: ShipBehaviour::SiphonDrone,
            },
        ));
    }
    for i in 0..NUM_SIPHON_SHUTTLES {
        ships.push((
            (7.0, (i as f64) / (NUM_SIPHON_SHUTTLES as f64)),
            ShipConfig {
                id: format!("{}/siphon_shuttle/{}", system_waypoint, i),
                ship_model: "SHIP_REFINING_FREIGHTER".to_string(),
                purchase_criteria: PurchaseCriteria {
                    system_symbol: Some(system_waypoint.clone()),
                    ..PurchaseCriteria::default()
                },
                behaviour: ShipBehaviour::SiphonShuttle,
            },
        ));
    }

    // Mining operation
    const NUM_SURVEYORS: i64 = 1;
    const NUM_MINING_DRONES: i64 = 4;
    const NUM_MINING_SHUTTLES: i64 = 1;
    for i in 0..NUM_SURVEYORS {
        ships.push((
            (3.0, (i as f64) / (NUM_SURVEYORS as f64)),
            ShipConfig {
                id: format!("{}/surveyor/{}", system_waypoint, i),
                ship_model: "SHIP_SURVEYOR".to_string(),
                purchase_criteria: PurchaseCriteria {
                    system_symbol: Some(system_waypoint.clone()),
                    ..PurchaseCriteria::default()
                },
                behaviour: ShipBehaviour::MiningSurveyor,
            },
        ));
    }
    for i in 0..NUM_MINING_DRONES {
        ships.push((
            (3.0, (i as f64) / (NUM_MINING_DRONES as f64)),
            ShipConfig {
                id: format!("{}/mining_drone/{}", system_waypoint, i),
                ship_model: "SHIP_ORE_HOUND".to_string(),
                purchase_criteria: PurchaseCriteria {
                    system_symbol: Some(system_waypoint.clone()),
                    ..PurchaseCriteria::default()
                },
                behaviour: ShipBehaviour::MiningDrone,
            },
        ));
    }
    for i in 0..NUM_MINING_SHUTTLES {
        ships.push((
            (3.0, (i as f64) / (NUM_MINING_SHUTTLES as f64)),
            ShipConfig {
                id: format!("{}/mining_shuttle/{}", system_waypoint, i),
                ship_model: "SHIP_REFINING_FREIGHTER".to_string(),
                purchase_criteria: PurchaseCriteria {
                    system_symbol: Some(system_waypoint.clone()),
                    ..PurchaseCriteria::default()
                },
                behaviour: ShipBehaviour::MiningShuttle,
            },
        ));
    }

    ships.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    ships.into_iter().map(|(_, c)| c).collect()
}
