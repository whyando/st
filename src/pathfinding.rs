use std::{collections::BTreeMap, sync::Arc};

use crate::models::{ShipFlightMode, Waypoint, WaypointSymbol};
use std::cmp::max;

#[allow(non_snake_case)]
const CRUISE_NAV_MODIFIER: f64 = 25.0;
const BURN_NAV_MODIFIER: f64 = 12.5;

#[derive(Debug)]
pub struct Pathfinding {
    waypoints: Arc<BTreeMap<WaypointSymbol, Waypoint>>,
    closest_market: BTreeMap<WaypointSymbol, (WaypointSymbol, i64)>,
}

pub struct Route {
    pub hops: Vec<(WaypointSymbol, Edge, bool, bool)>,
    pub min_travel_duration: i64,
    pub req_terminal_fuel: i64,
}

impl Pathfinding {
    pub fn new(waypoints: Vec<Waypoint>) -> Pathfinding {
        let mut waypoint_map: BTreeMap<WaypointSymbol, Waypoint> = BTreeMap::new();
        let mut closest_market: BTreeMap<WaypointSymbol, (WaypointSymbol, i64)> = BTreeMap::new();
        for waypoint in &waypoints {
            waypoint_map.insert(waypoint.symbol.clone(), waypoint.clone());
            if waypoint.is_market() {
                continue;
            }
            let closest = waypoints
                .iter()
                .filter(|w| w.is_market())
                .map(|w| {
                    let dist = distance(&waypoint, w);
                    (w.symbol.clone(), dist)
                })
                .min_by_key(|(_symbol, distance)| *distance)
                .unwrap();
            closest_market.insert(waypoint.symbol.clone(), closest);
        }
        Pathfinding {
            waypoints: Arc::new(waypoint_map),
            closest_market,
        }
    }

    pub fn estimate_duration_matrix(
        &self,
        speed: i64,
        _fuel_capacity: i64,
    ) -> BTreeMap<WaypointSymbol, BTreeMap<WaypointSymbol, i64>> {
        let mut duration_matrix: BTreeMap<WaypointSymbol, BTreeMap<WaypointSymbol, i64>> =
            BTreeMap::new();
        for src in self.waypoints.values() {
            let src_map = duration_matrix.entry(src.symbol.clone()).or_default();
            for dest in self.waypoints.values() {
                if src.symbol == dest.symbol {
                    src_map.insert(dest.symbol.clone(), 0);
                    continue;
                }
                let distance = distance(&src, &dest);
                let travel_duration = (15.0
                    + CRUISE_NAV_MODIFIER / (speed as f64) * (distance as f64))
                    .round() as i64;
                src_map.insert(dest.symbol.clone(), travel_duration);
            }
        }
        duration_matrix
    }

    pub fn get_route(
        &self,
        src_symbol: &WaypointSymbol,
        dest_symbol: &WaypointSymbol,
        speed: i64,
        start_fuel: i64, // ruins the cacheability slightly, since the graph changes
        fuel_capacity: i64,
    ) -> Route {
        use pathfinding::directed::dijkstra::dijkstra;

        let src = self.waypoints.get(src_symbol).unwrap();
        let dst = self.waypoints.get(dest_symbol).unwrap();
        let dest_is_market = dst.is_market();
        let src_is_market = src.is_market();
        let req_escape_fuel = if !dst.is_market() {
            let closest = self.closest_market.get(dest_symbol).unwrap();
            closest.1 // assumes CRUISE
        } else {
            0
        };

        // Route edge conditions:
        // - if src is not a market: the first hop must be <= start_fuel
        // - if dest is not a market, with the closest market X away,
        //   then the last hop must be <= max_fuel - X from a market
        //                          or <= start_fuel - X from a non-market src
        let path: (Vec<WaypointSymbol>, i64) = dijkstra(
            src_symbol,
            |x_symbol| {
                let x = self.waypoints.get(x_symbol).unwrap();
                // start with market <-> market edges
                let mut edges = if x.is_market() {
                    self.waypoints
                        .iter()
                        .filter(|(_y_symbol, y)| y.is_market())
                        .filter_map(|(y_symbol, y)| {
                            if x_symbol == y_symbol {
                                return None;
                            }
                            if let Some(e) = edge(x, y, speed, fuel_capacity) {
                                Some((y_symbol.clone(), e.travel_duration))
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<_>>()
                } else {
                    vec![]
                };
                // add non-market -> market edges ( fuel_cost <= start_fuel )
                if !src_is_market && x_symbol == src_symbol {
                    let edges1 = self
                        .waypoints
                        .iter()
                        .filter(|(_y_symbol, y)| y.is_market())
                        .filter_map(|(y_symbol, y)| {
                            if let Some(e) = edge(x, y, speed, start_fuel) {
                                Some((y_symbol.clone(), e.travel_duration))
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<_>>();
                    edges.extend(edges1);
                }
                // add market -> non-market edge ( fuel_cost <= max_fuel - req_escape_fuel )
                if !dest_is_market && x_symbol != dest_symbol {
                    if let Some(e) = edge(x, dst, speed, fuel_capacity - req_escape_fuel) {
                        edges.push((dest_symbol.clone(), e.travel_duration));
                    }
                }
                // finally add non-market -> non-market edge ( fuel_cost <= start_fuel - req_escape_fuel )
                if !src_is_market && !dest_is_market && x_symbol == src_symbol {
                    if let Some(e) = edge(src, dst, speed, start_fuel - req_escape_fuel) {
                        edges.push((dest_symbol.clone(), e.travel_duration));
                    }
                }
                edges
            },
            |x_symbol| *x_symbol == *dest_symbol,
        )
        .expect("No path found");

        let hops = path
            .0
            .iter()
            .zip(path.0.iter().skip(1))
            .map(|(a_symbol, b_symbol)| {
                let a = self.waypoints.get(a_symbol).unwrap();
                let b = self.waypoints.get(b_symbol).unwrap();
                let fuel_max = match (a.is_market(), b.is_market()) {
                    (true, true) => fuel_capacity,
                    (true, false) => fuel_capacity - req_escape_fuel,
                    (false, true) => start_fuel,
                    (false, false) => start_fuel - req_escape_fuel,
                };
                let e = edge(a, b, speed, fuel_max).unwrap();
                (b_symbol.clone(), e, a.is_market(), b.is_market())
            })
            .collect();
        Route {
            hops,
            min_travel_duration: path.1,
            req_terminal_fuel: req_escape_fuel,
        }
    }
}

fn distance(a: &Waypoint, b: &Waypoint) -> i64 {
    let distance2 = (a.x - b.x).pow(2) + (a.y - b.y).pow(2);
    max(1, (distance2 as f64).sqrt().round() as i64)
}

pub struct Edge {
    pub distance: i64,
    pub travel_duration: i64,
    pub fuel_cost: i64,
    pub flight_mode: ShipFlightMode,
}

pub fn edge(a: &Waypoint, b: &Waypoint, speed: i64, fuel_max: i64) -> Option<Edge> {
    let distance = distance(a, b);

    // burn
    if 2 * distance <= fuel_max {
        let travel_duration =
            (15.0 + BURN_NAV_MODIFIER / (speed as f64) * (distance as f64)).round() as i64;
        return Some(Edge {
            distance,
            travel_duration,
            fuel_cost: 2 * distance,
            flight_mode: ShipFlightMode::Burn,
        });
    }

    // cruise
    if distance <= fuel_max {
        let travel_duration =
            (15.0 + CRUISE_NAV_MODIFIER / (speed as f64) * (distance as f64)).round() as i64;
        return Some(Edge {
            distance,
            travel_duration,
            fuel_cost: distance,
            flight_mode: ShipFlightMode::Cruise,
        });
    }
    None
}
