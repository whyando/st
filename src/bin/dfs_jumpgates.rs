use futures::future::BoxFuture;
use st::agent_controller::AgentController;
use st::api_client::ApiClient;
use st::db::DbClient;
use st::models::WaypointSymbol;
use st::universe::Universe;
use std::collections::HashMap;
use std::env;
use std::io;
use std::sync::Arc;

#[tokio::main]
async fn main() -> io::Result<()> {
    dotenvy::dotenv().ok();
    pretty_env_logger::init_timed();

    let callsign = env::var("AGENT_CALLSIGN").expect("AGENT_CALLSIGN env var not set");

    let api_client = ApiClient::new();
    let status = api_client.status().await;

    // Use the reset date on the status response as a unique identifier to partition data between resets
    let db = DbClient::new(&status.reset_date).await;
    let agent_token = db.get_agent_token(&callsign).await.unwrap();
    api_client.set_agent_token(&agent_token);
    let universe = Arc::new(Universe::new(&api_client, &db));

    let agent_controller = AgentController::new(&api_client, &db, &universe, &callsign).await;

    // let all_systems = universe.all_systems().await;
    let root_gate = universe
        .get_jumpgate(&agent_controller.starting_system())
        .await;

    // recursively explore the universe
    let mut dfs = Dfs::new(&universe).await;
    dfs.explore(root_gate).await;
    dfs.result();

    Ok(())
}

struct Dfs {
    universe: Arc<Universe>,
    state: HashMap<WaypointSymbol, i32>,
}

impl Dfs {
    async fn new(universe: &Arc<Universe>) -> Self {
        Self {
            universe: universe.clone(),
            state: HashMap::new(),
        }
    }

    fn explore(&mut self, x: WaypointSymbol) -> BoxFuture<'_, ()> {
        Box::pin(self._explore(x))
    }

    async fn _explore(&mut self, x: WaypointSymbol) {
        if self.state.contains_key(&x) {
            return;
        }
        let _gate = self.universe.get_jumpgate_connections(&x).await;
        // get_jumpgate_connections should now only be called on charted jumpgates
        // todo: fix this with new interface
        todo!();
        // if let Some(gate) = connections {
        //     self.state.insert(x.clone(), gate.connections.len() as i32);
        //     if gate.is_constructed {
        //         for y in gate.connections {
        //             self.explore(y).await;
        //         }
        //     } else {
        //         log::debug!("Jumpgate {} is under construction", x);
        //     }
        // } else {
        //     self.state.insert(x.clone(), 0);
        //     log::debug!("Jumpgate {} connections unknown", x);
        // }
    }

    fn result(&self) {
        log::info!("Reachable: {}", self.state.len());
        let num_charted = self.state.values().filter(|&&v| v > 0).count();
        log::info!("Charted: {}", num_charted);
        let num_uncharted = self.state.values().filter(|&&v| v == 0).count();
        log::info!("Uncharted: {}", num_uncharted);
    }
}
