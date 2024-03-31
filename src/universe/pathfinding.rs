use super::Universe;
use crate::models::WaypointSymbol;
use std::collections::BTreeMap;

pub struct JumpGate {
    pub active_connections: Vec<(WaypointSymbol, i64)>,
    pub is_constructed: bool,
    pub all_connections_known: bool,
}

impl Universe {
    // Construct a map containing every jumpgate and its traversable connections
    pub async fn jumpgate_graph(&self) -> BTreeMap<WaypointSymbol, JumpGate> {
        // Start by loading the list of jumpgates from system list
        // Load the detailed system waypoints and the jumpgate connections if the waypoint is charted
        // This also tells us which jumpgates are constructed
        let jumpgates = self
            .systems()
            .into_iter()
            .filter(|s| !s.waypoints.is_empty())
            .filter_map(|s| {
                let filtered = s
                    .waypoints
                    .iter()
                    .filter(|w| w.waypoint_type == "JUMP_GATE")
                    .map(|w| w.symbol.clone())
                    .collect::<Vec<_>>();
                match filtered.len() {
                    0 => None,
                    1 => Some(filtered.first().unwrap().clone()),
                    _ => panic!("Multiple jumpgates in system {}", s.symbol),
                }
            })
            .collect::<Vec<_>>();
        let mut waypoints = BTreeMap::new();
        for waypoint_symbol in &jumpgates {
            let waypoint = self.detailed_waypoint(&waypoint_symbol).await;
            if !waypoint.is_uncharted() {
                let _gate = self.get_jumpgate_connections(&waypoint_symbol).await;
            }
            waypoints.insert(waypoint_symbol, waypoint);
        }

        // Read connections from self.jumpgates (includes uncharted gates that we know the connections for)
        let mut graph: BTreeMap<WaypointSymbol, JumpGate> = BTreeMap::new();
        for (&waypoint_symbol, waypoint) in waypoints.iter() {
            let known_connections = self.jumpgates.contains_key(&waypoint_symbol);
            let gate = JumpGate {
                active_connections: vec![],
                is_constructed: !waypoint.is_under_construction,
                all_connections_known: known_connections,
            };
            graph.insert(waypoint_symbol.clone(), gate);
        }
        for kv in self.jumpgates.iter() {
            let (src_symbol, jump_info) = kv.pair();
            let src_waypoint = waypoints.get(&src_symbol).unwrap();
            if src_waypoint.is_under_construction {
                continue;
            }
            for dst_symbol in &jump_info.connections {
                let dst_waypoint = waypoints.get(&dst_symbol).unwrap();
                if dst_waypoint.is_under_construction {
                    continue;
                }
                let distance = src_waypoint.distance(&dst_waypoint);
                let cooldown = 60 + distance;

                // src -> dst
                let src_entry = graph.get_mut(src_symbol).unwrap();
                src_entry
                    .active_connections
                    .push((dst_symbol.clone(), cooldown));

                // for dst -> src, insert unless 'complete'
                let entry = graph.get_mut(dst_symbol).unwrap();
                if !entry.all_connections_known {
                    entry
                        .active_connections
                        .push((src_symbol.clone(), cooldown));
                }
            }
        }

        graph
    }
}
