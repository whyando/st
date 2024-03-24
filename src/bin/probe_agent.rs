use reqwest::StatusCode;
use serde_json::Value;
use st::api_client::ApiClient;
use st::data::DataClient;
use std::env;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    pretty_env_logger::init_timed();

    let target = "SG-1-DEVX89";
    let callsign = env::var("AGENT_CALLSIGN")
        .expect("AGENT_CALLSIGN env var not set")
        .to_ascii_uppercase();

    let api_client = ApiClient::new();
    let status = api_client.status().await;

    // Use the reset date on the status response as a unique identifier to partition data between resets
    let db = DataClient::new(&status.reset_date).await;

    // Startup Phase: register if not already registered, and load agent token
    let agent_token = match db.get_agent_token(&callsign).await {
        Some(token) => token,
        None => panic!("Agent not registered"),
    };
    log::info!("Setting token {}", agent_token);
    api_client.set_agent_token(&agent_token);

    for i in 1..=213 {
        // format as hex
        let uri = format!("/my/ships/{}-{:X}", target, i);
        let (code, resp_body): (StatusCode, Result<Value, String>) = api_client
            .request(reqwest::Method::GET, &uri, None::<&()>)
            .await;
        let resp = resp_body.unwrap_err();
        println!("{} {:?} {:?}", uri, code, resp);
    }
}
