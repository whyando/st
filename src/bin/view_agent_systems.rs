use serde::{Deserialize, Serialize};
use st::api_client::ApiClient;
use st::models::WaypointSymbol;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Agent {
    pub symbol: String,
    pub headquarters: WaypointSymbol,
    pub credits: i64,
    pub starting_faction: String,
    pub ship_count: u32,
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    pretty_env_logger::init_timed();

    let api_client = ApiClient::new();
    // {"symbol":"05HD3ITEFVHT","headquarters":"X1-SZ63-A1","credits":175000,"startingFaction":"COSMIC","shipCount":2}
    let agents: Vec<Agent> = api_client.get_all_pages("/agents").await;
    let mut factions = std::collections::BTreeMap::new();
    let mut headquarters = std::collections::BTreeMap::new();

    for agent in agents {
        let faction = factions
            .entry(agent.starting_faction.clone())
            .or_insert((0, 0, 0));
        let val = format!("{}-{}", agent.starting_faction, agent.headquarters);
        let hq = headquarters.entry(val).or_insert((0, 0, 0));

        if agent.credits <= 175000 && agent.ship_count == 2 {
            continue;
        }

        faction.0 += 1;
        faction.1 += agent.credits;
        faction.2 += agent.ship_count;

        hq.0 += 1;
        hq.1 += agent.credits;
        hq.2 += agent.ship_count;
    }

    println!("Factions:");
    for (faction, (count, credits, ship_count)) in factions {
        println!(
            "{}: {} agents, {} credits, {} ships",
            faction, count, credits, ship_count
        );
    }
    println!("\nHeadquarters:");
    for (hq, (count, credits, ship_count)) in headquarters {
        println!(
            "{}: {} agents, {} credits, {} ships",
            hq, count, credits, ship_count
        );
    }
}
