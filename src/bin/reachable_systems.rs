use log::*;
use pathfinding::prelude::*;
use st::api_client::ApiClient;
use st::db::DbClient;
use st::universe::Universe;
use std::env;
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
    let system = agent.headquarters.system();
    let start = universe.get_jumpgate(&system).await;

    let graph = universe.jumpgate_graph().await;

    let reachables = dijkstra_all(&start, |node| {
        graph.get(node).unwrap().active_connections.clone()
    });
    let mut reachable_systems = Vec::new();
    for (system, distance) in &reachables {
        reachable_systems.push((system.clone(), distance));
    }
    reachable_systems.sort_by_key(|(_system, (_, d))| *d);

    for (system, (_pre, cd)) in &reachable_systems {
        let cd_hours = cd / 3600;
        let cd_minutes = (cd % 3600) / 60;
        let cd_seconds = cd % 60;
        info!("{}: {}h {}m {}s", system, cd_hours, cd_minutes, cd_seconds);
        // let path = build_path(&system, &reachables);
        // let route = path
        //     .iter()
        //     .map(|s| s.to_string())
        //     .collect::<Vec<_>>()
        //     .join(" -> ");
        // info!("  {}", route);
    }
    info!(
        "Total reachable gates: {}/{}",
        reachable_systems.len() + 1,
        graph.len()
    );

    Ok(())
}
