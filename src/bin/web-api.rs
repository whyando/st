use st::{api_client::ApiClient, data::DataClient, models::SystemSymbol};
use std::env;
use warp::hyper::Body;
use warp::{reply::Response, Filter};

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    pretty_env_logger::init_timed();

    let callsign = env::var("AGENT_CALLSIGN")
        .expect("AGENT_CALLSIGN env var not set")
        .to_ascii_uppercase();

    let api_client = ApiClient::new();
    let status = api_client.status().await;
    let db_client = DataClient::new(&status.reset_date).await;
    let agent_public = api_client.get_agent_public(&callsign).await;

    let reset = warp::path!("api" / "reset_date")
        .map(move || Response::new(Body::from(status.reset_date.clone())));
    let agent = {
        let agent_public = agent_public.clone();
        warp::path!("api" / "agent").map(move || warp::reply::json(&agent_public))
    };
    let waypoints = {
        let system_symbol = agent_public.headquarters.system();
        warp::path!("api" / "waypoints")
            .and(warp::any().map(move || db_client.clone()))
            .and(warp::any().map(move || system_symbol.clone()))
            .then(
                |db_client: DataClient, system_symbol: SystemSymbol| async move {
                    let waypoints = db_client.get_system_waypoints(&system_symbol).await;
                    warp::reply::json(&waypoints)
                },
            )
    };

    warp::serve(reset.or(agent).or(waypoints))
        .run(([0, 0, 0, 0], 3030))
        .await;
}
