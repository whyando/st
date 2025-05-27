use log::*;
use reqwest::StatusCode;
use st::agent_controller::AgentController;
use st::api_client::ApiClient;
use st::config::CONFIG;
use st::database::DbClient;
use st::models::Faction;
use st::universe::Universe;
use std::env;
use std::sync::Arc;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    pretty_env_logger::init_timed();

    let spacetraders_env = env::var("SPACETRADERS_ENV").unwrap();
    let faction = env::var("AGENT_FACTION").unwrap_or("".to_string());
    let callsign = env::var("AGENT_CALLSIGN")
        .expect("AGENT_CALLSIGN env var not set")
        .to_ascii_uppercase();

    info!("Starting agent {} for faction {}", callsign, faction);
    info!("Loaded config: {:?}", *CONFIG);

    let api_client = ApiClient::new();
    let (status_code, status) = api_client.status().await;
    let status = match status_code {
        StatusCode::OK => status.unwrap(),
        _ => {
            error!("Failed to get status: {}\nbody: {:?}", status_code, status);
            panic!("Failed to get status");
            // TODO: handle 503 maintenance mode by repeating
        }
    };

    info!("Spacetraders env: {:?}", spacetraders_env);
    info!("Reset date: {:?}", status.reset_date);

    // Use the reset date on the status response as a unique identifier to partition data between resets
    let db = DbClient::new(&spacetraders_env, &status.reset_date).await;

    let universe = Arc::new(Universe::new(&api_client, &db));
    universe.init().await;

    // Startup Phase: register if not already registered, and load agent token
    let agent_token = match db.get_agent_token(&callsign).await {
        Some(token) => token,
        None => {
            let faction = match faction.as_str() {
                "" => {
                    // Pick a random faction
                    let factions: Vec<Faction> = api_client.get_all_pages("/factions").await;
                    let factions: Vec<Faction> =
                        factions.into_iter().filter(|f| f.is_recruiting).collect();
                    use rand::prelude::IndexedRandom as _;
                    let faction = factions.choose(&mut rand::rng()).unwrap();
                    info!("Picked faction {}", faction.symbol);
                    faction.symbol.clone()
                }
                _ => faction.to_string(),
            };
            let token = api_client.register(&faction, &callsign).await;
            db.save_agent_token(&callsign, &token).await;
            token
        }
    };
    log::info!("Setting token {}", agent_token);
    api_client.set_agent_token(&agent_token);

    let agent_controller = AgentController::new(&api_client, &db, &universe, &callsign).await;
    agent_controller.run_ships().await;
}
