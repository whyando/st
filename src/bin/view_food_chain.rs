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
    let mut markets = Vec::new();
    for waypoint in &waypoints {
        if waypoint.is_market() {
            if let Some(market) = universe.get_market(&waypoint.symbol).await {
                markets.push(market);
            }
        }
    }
    for market in markets {
        println!("Market: {}", market.data.symbol);
        for trade_good in &market.data.trade_goods {
            let activity = match &trade_good.activity {
                Some(x) => x.to_string(),
                None => "".to_string(),
            };
            println!(
                "   {}\t{}\t{}\t{}\t{}",
                trade_good.symbol,
                trade_good._type,
                trade_good.supply,
                activity,
                trade_good.trade_volume
            );
        }
        println!();
    }
}
