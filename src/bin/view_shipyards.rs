use st::agent_controller::AgentController;
use st::api_client::ApiClient;
use st::data::DataClient;
use st::models::Waypoint;
use st::universe::Universe;
use std::env;

#[tokio::main]
async fn main() {
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
    let system_symbol = agent_controller.starting_system();

    let waypoints: Vec<Waypoint> = universe.get_system_waypoints(&system_symbol).await;
    let mut shipyards = Vec::new();
    for waypoint in &waypoints {
        if waypoint.is_shipyard() {
            if let Some(shipyard) = universe.get_shipyard(&waypoint.symbol).await {
                shipyards.push(shipyard);
            }
        }
    }
    for shipyard in shipyards {
        println!("Shipyard: {}", shipyard.data.symbol);
        for ship in &shipyard.data.ships {
            println!("{} ${}", ship.ship_type, ship.purchase_price);
            println!("supply: {}", ship.supply);
            println!(
                "frame: {} ({} fuel)",
                ship.frame.symbol, ship.frame.fuel_capacity
            );
            println!("reactor: {}", ship.reactor.symbol);
            println!("engine: {} ({})", ship.engine.symbol, ship.engine.speed);
            let modules = ship
                .modules
                .iter()
                .map(|m| m.symbol.clone())
                .collect::<Vec<String>>()
                .join(", ");
            println!("modules: {}", modules);
            let mounts = ship
                .mounts
                .iter()
                .map(|m| m.symbol.clone())
                .collect::<Vec<String>>()
                .join(", ");
            println!("mounts: {}", mounts);
            let cargo_capacity = ship
                .modules
                .iter()
                .filter(|m| m.symbol.starts_with("MODULE_CARGO_HOLD_"))
                .map(|m| m.capacity.unwrap_or(0))
                .sum::<i64>();
            println!("cargo capacity: {}", cargo_capacity);

            println!();
        }
    }
}
