use pathfinding::prelude::*;
use st::api_client::ApiClient;
use st::db::DbClient;
// use st::universe::pathfinding::EdgeType;
use st::universe::Universe;
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
    starter_systems = starter_systems.into_iter().take(50).collect();

    // output to ./starter_systems.txt
    let mut f = File::create("starter_systems.txt")?;
    use std::io::Write as _;
    for (system, data) in starter_systems {
        match data {
            Some((&cd, _path)) => {
                let cd_hours = cd / 3600;
                let cd_minutes = (cd % 3600) / 60;
                let cd_seconds = cd % 60;
                writeln!(
                    &mut f,
                    "{}: {}h {}m {}s",
                    system, cd_hours, cd_minutes, cd_seconds
                )?;
                // for pair in path.windows(2) {
                //     let s = &pair[0];
                //     let t = &pair[1];
                //     let edge = &graph[s][t];
                //     let type_ = match edge.edge_type {
                //         EdgeType::Warp => "W",
                //         EdgeType::Jumpgate => "J",
                //     };
                //     writeln!(&mut f, "\t{} {} {}", type_, t, edge.duration)?
                // }
            }
            None => {
                writeln!(&mut f, "{}: unreachable", system)?;
            }
        }
    }
    Ok(())
}
