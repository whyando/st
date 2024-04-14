use pathfinding::prelude::*;
use st::api_client::ApiClient;
use st::db::DbClient;
use st::universe::pathfinding::EdgeType;
use st::universe::Universe;
use std::cmp::min;
use std::env;
use std::fs::File;
use std::io;
use std::sync::Arc;

#[tokio::main]
async fn main() -> io::Result<()> {
    dotenvy::dotenv().ok();
    pretty_env_logger::init_timed();

    let callsign = env::var("AGENT_CALLSIGN").expect("AGENT_CALLSIGN env var not set");

    let api_client = ApiClient::new();
    let status = api_client.status().await;
    let db = DbClient::new(&status.reset_date).await;
    let universe = Arc::new(Universe::new(&api_client, &db));
    universe.init().await;

    let agent = api_client.get_agent_public(&callsign).await;
    let start = universe
        .get_faction(&agent.starting_faction)
        .await
        .headquarters
        .unwrap();

    let graph = universe.warp_jump_graph().await;

    let reachables = dijkstra_all(&start, |node| {
        graph
            .get(node)
            .unwrap()
            .iter()
            .map(|(s, d)| (s.clone(), d.duration))
    });

    let mut starter_systems = vec![];
    for system in universe.systems() {
        if !system.is_starter_system() {
            continue;
        }
        let system_symbol = system.symbol.clone();
        match reachables.get(&system_symbol) {
            Some((_pre, cd)) => {
                let path = build_path(&system_symbol, &reachables);
                starter_systems.push((system_symbol, Some((cd, path))));
            }
            None => {
                starter_systems.push((system_symbol, None));
            }
        }
    }
    starter_systems.sort_by_key(|(_system, data)| match &data {
        Some((cd, _path)) => *cd,
        None => &i64::MAX,
    });
    // starter_systems = starter_systems.into_iter().take(100).collect();

    // output to ./starter_systems.txt
    let mut f = File::create("starter_systems.txt")?;
    use std::io::Write as _;
    for (idx, (system, data)) in starter_systems.iter().enumerate() {
        match data {
            Some((&cd, path)) => {
                let cd_hours = cd / 3600;
                let cd_minutes = (cd % 3600) / 60;
                let cd_seconds = cd % 60;
                writeln!(
                    &mut f,
                    "{}. {}: {}h {}m {}s",
                    idx, system, cd_hours, cd_minutes, cd_seconds
                )?;
                const MAX_FUEL: i64 = 800;
                const MAX_CARGO_FUEL: i64 = 40;
                let mut fuel: i64 = MAX_FUEL;
                let mut cargo_fuel: i64 = MAX_CARGO_FUEL;
                let mut out_of_fuel: bool = false;
                for pair in path.windows(2) {
                    let s = &pair[0];
                    let t = &pair[1];
                    let edge = &graph[s][t];
                    let type_ = match edge.edge_type {
                        EdgeType::Warp => "W",
                        EdgeType::Jumpgate => "J",
                    };
                    match edge.edge_type {
                        EdgeType::Warp => {
                            // refuel to at least edge.fuel
                            let refuel = {
                                let missing_fuel = MAX_FUEL - fuel;
                                // round down to the nearest 100, so we don't buy more than we need
                                let units = (missing_fuel / 100) * 100;
                                if units + fuel < edge.fuel {
                                    missing_fuel
                                } else {
                                    units
                                }
                            };
                            let refuel = min(refuel, cargo_fuel * 100);
                            let fuel_cargo_units = (refuel + 99) / 100;
                            fuel += refuel - edge.fuel;
                            cargo_fuel -= fuel_cargo_units;
                            if fuel < 0 || cargo_fuel < 0 {
                                out_of_fuel = true;
                            }
                        }
                        EdgeType::Jumpgate => {
                            fuel = MAX_FUEL;
                            cargo_fuel = MAX_CARGO_FUEL;
                        }
                    }
                    writeln!(
                        &mut f,
                        "\t{} {} {}f {}s",
                        type_, t, edge.fuel, edge.duration
                    )?
                }
                if out_of_fuel {
                    writeln!(&mut f, "\tOUT OF FUEL")?;
                }
            }
            None => {
                writeln!(&mut f, "{}. {}: unreachable", idx, system)?;
            }
        }
    }
    Ok(())
}
