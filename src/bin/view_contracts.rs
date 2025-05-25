// use tracing::*;
// //use st::agent_controller::AgentController;
// use st::api_client::ApiClient;
// use st::db::DbClient;
// use st::models::*;
// //use st::universe::Universe;
// use std::env;

// #[tokio::main]
// async fn main() {
//     dotenvy::dotenv().ok();
//     pretty_env_logger::init_timed();

//     let callsign = env::var("AGENT_CALLSIGN").expect("AGENT_CALLSIGN env var not set");

//     let api_client = ApiClient::new();
//     let status = api_client.status().await;

//     // Use the reset date on the status response as a unique identifier to partition data between resets
//     let db = DbClient::new(&status.reset_date).await;
//     let agent_token = db.get_agent_token(&callsign).await.unwrap();
//     api_client.set_agent_token(&agent_token);
//     // let universe = Universe::new(&api_client, &db);

//     // let agent_controller = AgentController::new(&api_client, &db, &universe, &callsign).await;
//     let contracts: Vec<Contract> = api_client.get_all_pages("/my/contracts").await;
//     info!("Contracts: {:?}", contracts);
// }
fn main() {}
