use log::*;
use st::agent_controller::AgentController;
use st::api_client::ApiClient;
use st::config::CONFIG;
use st::db::DbClient;
use st::universe::Universe;
use st::web_api_server::WebApiServer;
use std::env;
use std::sync::Arc;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    pretty_env_logger::init_timed();

    let faction = env::var("AGENT_FACTION").unwrap_or("".to_string());
    let callsign = env::var("AGENT_CALLSIGN")
        .expect("AGENT_CALLSIGN env var not set")
        .to_ascii_uppercase();
    let email = env::var("AGENT_EMAIL").ok();

    info!("Starting agent {} for faction {}", callsign, faction);
    info!("Loaded config: {:?}", *CONFIG);

    let api_client = ApiClient::new();
    let status = api_client.status().await;

    // Use the reset date on the status response as a unique identifier to partition data between resets
    let db = DbClient::new(&status.reset_date).await;
    let universe = Arc::new(Universe::new(&api_client, &db));
    universe.init().await;

    // Startup Phase: register if not already registered, and load agent token
    let agent_token = match db.get_agent_token(&callsign).await {
        Some(token) => token,
        None => {
            let token = api_client.register(&faction, &callsign, email).await;
            db.save_agent_token(&callsign, &token).await;
            token
        }
    };
    log::info!("Setting token {}", agent_token);
    api_client.set_agent_token(&agent_token);

    let agent_controller = AgentController::new(&api_client, &db, &universe, &callsign).await;
    let api_server = WebApiServer::new(&agent_controller, &db, &universe);
    tokio::join!(agent_controller.run_ships(), api_server.run());
}
