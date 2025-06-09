use prettytable::{format, row, Table};
use st::api_client::api_models::WaypointDetailed;
use st::api_client::ApiClient;
use st::database::DbClient;

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
    let status = api_client.status().await.1.unwrap();
    let db = DbClient::new(&status.reset_date).await;
    let universe = Arc::new(Universe::new(&api_client, &db).await);

    let agent = api_client.get_agent_public(&callsign).await;
    let system_symbol = agent.headquarters.system();

    let waypoints: Vec<WaypointDetailed> = universe.get_system_waypoints(&system_symbol).await;
    let mut markets = Vec::new();
    for waypoint in &waypoints {
        if waypoint.is_market() {
            if let Some(market) = universe.get_market(&waypoint.symbol) {
                markets.push(market);
            }
        }
    }

    // output to ./markets.txt
    let mut f = File::options()
        .write(true)
        .create(true)
        .truncate(true)
        .open("markets.txt")
        .unwrap();
    use std::io::Write as _;

    for market in markets {
        writeln!(&mut f, "Market: {}", market.data.symbol)?;

        let mut table = Table::new();
        table.set_format(*format::consts::FORMAT_NO_LINESEP_WITH_TITLE);
        table.add_row(row![
            "Symbol",
            "Type",
            "Supply",
            "Activity",
            "Volume",
            "Buy Price",
            "Sell Price"
        ]);

        for trade_good in &market.data.trade_goods {
            let activity = match &trade_good.activity {
                Some(x) => x.to_string(),
                None => "".to_string(),
            };
            table.add_row(row![
                trade_good.symbol,
                trade_good._type,
                trade_good.supply,
                activity,
                trade_good.trade_volume,
                format!("${}", trade_good.purchase_price),
                format!("${}", trade_good.sell_price)
            ]);
        }

        writeln!(&mut f, "{}", table)?;
        writeln!(&mut f)?;
    }
    log::info!("Wrote markets to markets.txt");
    Ok(())
}
