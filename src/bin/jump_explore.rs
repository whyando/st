use futures::future::BoxFuture;
use st::agent_controller::AgentController;
use st::api_client::ApiClient;
use st::data::DataClient;
use st::models::WaypointSymbol;
use st::universe::JumpGateConnections;
use st::universe::Universe;
use std::collections::HashMap;
use std::env;
use std::io;

#[tokio::main]
async fn main() -> io::Result<()> {
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

    // let all_systems = universe.all_systems().await;
    let root_gate = universe
        .get_jumpgate(&agent_controller.starting_system())
        .await;

    // recursively explore the universe
    let mut dfs = Dfs::new(universe).await;
    dfs.explore(root_gate).await;
    dfs.result();

    Ok(())
}

struct Dfs {
    universe: Universe,
    state: HashMap<WaypointSymbol, i32>,
}

impl Dfs {
    async fn new(universe: Universe) -> Self {
        Self {
            universe,
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
        let connections = self.universe.get_jumpgate_connections(&x).await;
        match connections.connections {
            JumpGateConnections::Charted(connections) => {
                self.state.insert(x.clone(), connections.len() as i32);
                for y in connections {
                    self.explore(y).await;
                }
            }
            JumpGateConnections::Uncharted => {
                self.state.insert(x.clone(), 0);
                log::debug!("Jumpgate {} is uncharted", x);
            }
        }
    }

    fn result(&self) {
        log::info!("Reachable: {}", self.state.len());
    }
}
