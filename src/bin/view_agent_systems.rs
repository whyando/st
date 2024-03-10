use std::fs::File;

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
async fn main() -> std::io::Result<()> {
    dotenvy::dotenv().ok();
    pretty_env_logger::init_timed();

    // output to ./all_agents.txt
    let mut f = File::options()
        .write(true)
        .create(true)
        .open("all_agents.txt")
        .unwrap();
    use std::io::Write as _;

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

        // if agent.credits <= 175000 && agent.ship_count == 2 {
        //     continue;
        // }
        writeln!(
            &mut f,
            "{}\t{}\t{}\t{}",
            agent.ship_count, agent.credits, agent.headquarters, agent.symbol
        )?;

        faction.0 += 1;
        faction.1 += agent.credits;
        faction.2 += agent.ship_count;

        hq.0 += 1;
        hq.1 += agent.credits;
        hq.2 += agent.ship_count;
    }

    writeln!(&mut f, "")?;
    writeln!(&mut f, "Factions:")?;
    for (faction, (count, credits, ship_count)) in factions {
        writeln!(
            &mut f,
            "{}: {} agents, {} credits, {} ships",
            faction, count, credits, ship_count
        )?;
    }
    writeln!(&mut f, "\nHeadquarters:")?;
    for (hq, (count, credits, ship_count)) in headquarters {
        writeln!(
            &mut f,
            "{}: {} agents, {} credits, {} ships",
            hq, count, credits, ship_count
        )?;
    }
    Ok(())
}
