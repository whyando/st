use log::info;
use st::agent_controller::AgentController;
use st::api_client::ApiClient;
use st::data::DataClient;
use st::models::{SystemSymbol, Waypoint};
use st::universe::Universe;
use std::env;
use std::fs::File;
use std::io;

#[tokio::main]
async fn main() -> io::Result<()> {
    dotenvy::dotenv().ok();
    pretty_env_logger::init_timed();

    let callsign = env::var("AGENT_CALLSIGN").expect("AGENT_CALLSIGN env var not set");

    let api_client = ApiClient::new();
    let status = api_client.status().await;

    // Use the reset date on the status response as a unique identifier to partition data between resets
    let db = DataClient::new(&status.reset_date).await;
    let agent_token = db.get_agent_token(&callsign).await.unwrap();
    api_client.set_agent_token(&agent_token);
    let universe = Universe::new(&api_client, &db);

    let agent_controller = AgentController::new(&api_client, &db, &universe, &callsign).await;
    // let system_symbol = agent_controller.starting_system();
    let system_symbol = SystemSymbol("X1-TZ54".to_string());

    let waypoints: Vec<Waypoint> = universe.get_system_waypoints(&system_symbol).await;
    let mut shipyards = Vec::new();
    let mut shipyards_remote = Vec::new();
    for waypoint in &waypoints {
        if waypoint.is_shipyard() {
            if let Some(shipyard) = universe.get_shipyard(&waypoint.symbol).await {
                shipyards.push(shipyard);
            }
            let shipyard_opt = universe.get_shipyard_remote(&waypoint.symbol).await;
            shipyards_remote.push(shipyard_opt);
        }
    }

    // output to ./shipyards.txt
    let mut f = File::options()
        .write(true)
        .create(true)
        .open("shipyards.txt")
        .unwrap();
    use std::io::Write as _;

    for shipyard in shipyards_remote {
        let ships = shipyard
            .ship_types
            .iter()
            .map(|s| s.ship_type.clone())
            .collect::<Vec<String>>()
            .join(", ");
        writeln!(&mut f, "Shipyard: {}", ships)?;
    }
    writeln!(&mut f, "")?;

    for shipyard in shipyards {
        writeln!(&mut f, "Shipyard: {}", shipyard.data.symbol)?;
        for ship in &shipyard.data.ships {
            writeln!(&mut f, "{} ${}", ship.ship_type, ship.purchase_price)?;
            writeln!(&mut f, "supply: {}", ship.supply)?;
            writeln!(
                &mut f,
                "frame: {} ({} fuel)",
                ship.frame.symbol, ship.frame.fuel_capacity
            )?;
            writeln!(&mut f, "reactor: {}", ship.reactor.symbol)?;
            writeln!(
                &mut f,
                "engine: {} ({})",
                ship.engine.symbol, ship.engine.speed
            )?;
            let modules = ship
                .modules
                .iter()
                .map(|m| m.symbol.clone())
                .collect::<Vec<String>>()
                .join(", ");
            writeln!(&mut f, "modules: {}", modules)?;
            let mounts = ship
                .mounts
                .iter()
                .map(|m| m.symbol.clone())
                .collect::<Vec<String>>()
                .join(", ");
            writeln!(&mut f, "mounts: {}", mounts)?;
            let cargo_capacity = ship
                .modules
                .iter()
                .filter(|m| m.symbol.starts_with("MODULE_CARGO_HOLD_"))
                .map(|m| m.capacity.unwrap_or(0))
                .sum::<i64>();
            writeln!(&mut f, "cargo capacity: {}", cargo_capacity)?;

            writeln!(&mut f,)?;
        }
    }
    info!("Wrote shipyards to shipyards.txt");
    Ok(())
}
