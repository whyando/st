use crate::broker::{CargoBroker, TransferActor};
use crate::models::{ShipNavStatus::*, *};
use crate::ship_config::ship_config;
use crate::survey_manager::SurveyManager;
use crate::{
    api_client::ApiClient,
    data::DataClient,
    models::{Agent, Ship, ShipBehaviour, ShipConfig, SystemSymbol, Waypoint, WaypointSymbol},
    ship_controller::ShipController,
    ship_scripts,
    tasks::LogisticTaskManager,
    universe::Universe,
};
use dashmap::DashMap;
use futures::future::BoxFuture;
use futures::stream::FuturesUnordered;
use log::*;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::ops::Deref;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct AgentController {
    universe: Universe,
    api_client: ApiClient,
    db: DataClient,

    callsign: String,
    agent: Arc<Mutex<Agent>>,
    ships: Arc<DashMap<String, Arc<Mutex<Ship>>>>,

    ship_config: Arc<Vec<ShipConfig>>,
    job_assignments: Arc<DashMap<String, String>>,
    job_assignments_rev: Arc<DashMap<String, String>>,
    // ship_futs: Arc<Mutex<VecDeque<tokio::task::JoinHandle<()>>>>,
    hdls: Arc<JoinHandles>,
    pub task_manager: Arc<LogisticTaskManager>,
    pub survey_manager: Arc<SurveyManager>,
    pub siphon_cargo_broker: Arc<CargoBroker>,

    try_buy_ships_mutex_guard: Arc<tokio::sync::Mutex<()>>,
}

impl TransferActor for AgentController {
    fn _transfer_cargo(
        &self,
        src_ship_symbol: String,
        dest_ship_symbol: String,
        good: String,
        units: i64,
    ) -> Pin<Box<dyn std::future::Future<Output = ()> + Send>> {
        let self_clone = self.clone();
        Box::pin(async move {
            self_clone
                .transfer_cargo(src_ship_symbol, dest_ship_symbol, good, units)
                .await;
        })
    }
}

impl AgentController {
    pub async fn transfer_cargo(
        &self,
        src_ship_symbol: String,
        dest_ship_symbol: String,
        good: String,
        units: i64,
    ) {
        debug!("agent_controller::transfer_cargo");
        let src_ship = self.ships.get(&src_ship_symbol).unwrap();
        let dest_ship = self.ships.get(&dest_ship_symbol).unwrap();

        self.debug(&format!(
            "Transferring {} -> {} {} {}",
            &src_ship_symbol, &dest_ship_symbol, &units, &good
        ));
        let uri = format!("/my/ships/{}/transfer", &src_ship_symbol);
        let body = json!({
            "shipSymbol": &dest_ship_symbol,
            "tradeSymbol": &good,
            "units": &units,
        });
        let mut response: Value = self.api_client.post(&uri, &body).await;
        let cargo: ShipCargo = serde_json::from_value(response["data"]["cargo"].take()).unwrap();
        let mut src_ship = src_ship.lock().unwrap();
        let mut dest_ship = dest_ship.lock().unwrap();

        let transferred: ShipCargoItem = {
            let mut x = src_ship
                .cargo
                .inventory
                .iter()
                .find(|x| x.symbol == good)
                .unwrap()
                .clone();
            x.units = units;
            x
        };
        src_ship.cargo = cargo;
        dest_ship.incr_cargo(transferred);
        debug!("agent_controller::transfer_cargo done");
    }

    pub async fn new(
        api_client: &ApiClient,
        db: &DataClient,
        universe: &Universe,
        callsign: &str,
    ) -> Self {
        // Load agent + ships
        let agent: Arc<Mutex<Agent>> = {
            let agent = api_client.get_agent().await;
            assert_eq!(agent.symbol, callsign);
            Arc::new(Mutex::new(agent))
        };
        let ships: Arc<DashMap<String, Arc<Mutex<Ship>>>> = {
            let ships_vec: Vec<Ship> = api_client.get_all_ships().await;
            let ships = Arc::new(DashMap::new());
            for ship in ships_vec {
                ships.insert(ship.symbol.clone(), Arc::new(Mutex::new(ship)));
            }
            ships
        };
        let system_symbol = agent.lock().unwrap().headquarters.system();
        let waypoints: Vec<Waypoint> = universe.get_system_waypoints(&system_symbol).await;
        // static ship config - later we may allow limited modifications
        let ship_config: Vec<ShipConfig> = ship_config(&waypoints);
        let job_assignments: DashMap<String, String> = db
            .get_value(&format!("{}/ship_assignments", callsign))
            .await
            .unwrap_or_default();
        let job_assignments_rev = job_assignments
            .iter()
            .map(|x| {
                let (k, v) = x.pair();
                (v.clone(), k.clone())
            })
            .collect();
        let task_manager = LogisticTaskManager::new(universe, db, &agent, &system_symbol).await;
        let survey_manager = SurveyManager::new(db).await;
        let agent_controller = Self {
            callsign: callsign.to_string(),
            agent,
            ships,
            api_client: api_client.clone(),
            db: db.clone(),
            universe: universe.clone(),
            // ship_futs: Arc::new(Mutex::new(VecDeque::new())),
            hdls: Arc::new(JoinHandles::new()),
            ship_config: Arc::new(ship_config),
            job_assignments: Arc::new(job_assignments),
            job_assignments_rev: Arc::new(job_assignments_rev),
            task_manager: Arc::new(task_manager),
            siphon_cargo_broker: Arc::new(CargoBroker::new()),
            survey_manager: Arc::new(survey_manager),
            try_buy_ships_mutex_guard: Arc::new(tokio::sync::Mutex::new(())),
        };
        agent_controller
            .task_manager
            .set_agent_controller(&agent_controller);
        let credits = agent_controller.credits();
        let num_ships = agent_controller.num_ships();
        info!(
            "Loaded agent {} ${} with {} ships",
            callsign, credits, num_ships
        );
        agent_controller
    }
    pub fn credits(&self) -> i64 {
        self.agent.lock().unwrap().credits
    }
    pub fn starting_system(&self) -> SystemSymbol {
        self.agent.lock().unwrap().headquarters.system()
    }
    pub fn num_ships(&self) -> usize {
        self.ships.len()
    }
    pub fn update_agent(&self, agent: Agent) {
        *self.agent.lock().unwrap() = agent;
    }
    fn debug(&self, msg: &str) {
        debug!("[{}] {}", self.callsign, msg);
    }

    pub fn probed_waypoints(&self) -> Vec<(String, WaypointSymbol)> {
        self.ship_config
            .iter()
            .filter_map(|job| {
                if let ShipBehaviour::FixedProbe(waypoint_symbol) = &job.behaviour {
                    if let Some(assignment) = self.job_assignments.get(&job.id) {
                        let ship = self.ships.get(assignment.value()).unwrap();
                        let ship = ship.lock().unwrap();
                        if ship.nav.status != InTransit
                            && ship.nav.waypoint_symbol == *waypoint_symbol
                        {
                            return Some((ship.symbol.clone(), waypoint_symbol.clone()));
                        }
                    }
                }
                None
            })
            .collect()
    }

    async fn buy_ship(&self, shipyard: &WaypointSymbol, ship_model: &str) -> String {
        self.debug(&format!("Buying {} at {}", &ship_model, &shipyard));
        let uri = "/my/ships";
        let body = json!({
            "shipType": ship_model,
            "waypointSymbol": shipyard,
        });
        let mut response: Value = self.api_client.post(uri, &body).await;
        let agent: Agent = serde_json::from_value(response["data"]["agent"].take()).unwrap();
        let ship: Ship = serde_json::from_value(response["data"]["ship"].take()).unwrap();
        // let transaction = response["data"]["transaction"].take();
        let ship_symbol = ship.symbol.clone();
        self.debug(&format!("Successfully bought ship {}", ship_symbol));
        self.update_agent(agent);
        self.ships
            .insert(ship_symbol.clone(), Arc::new(Mutex::new(ship)));
        ship_symbol
    }

    pub fn ship_controller(&self, ship_symbol: &str) -> ShipController {
        let ship = self.ships.get(ship_symbol).unwrap();
        ShipController::new(
            &self.api_client,
            &self.db,
            &self.universe,
            ship.clone(),
            self,
        )
    }
    pub fn ship_assigned(&self, ship_symbol: &str) -> bool {
        self.job_assignments_rev.contains_key(ship_symbol)
    }
    pub fn job_assigned(&self, job_id: &str) -> bool {
        self.job_assignments.contains_key(job_id)
    }

    async fn try_buy_ships_lock(&self) -> tokio::sync::MutexGuard<()> {
        match self.try_buy_ships_mutex_guard.try_lock() {
            Ok(guard) => guard,
            Err(_e) => {
                debug!("AgentController::try_buy_ships is already running");
                let timeout = tokio::time::Duration::from_secs(30);
                match tokio::time::timeout(timeout, self.try_buy_ships_mutex_guard.lock()).await {
                    Ok(guard) => {
                        debug!("AgentController::try_buy_ships lock acquired");
                        guard
                    }
                    Err(_e) => {
                        panic!("AgentController::try_buy_ships lock timeout");
                    }
                }
            }
        }
    }

    pub async fn try_buy_ships(
        &self,
        purchaser: Option<String>,
    ) -> (Vec<String>, BTreeSet<WaypointSymbol>) {
        let _guard = self.try_buy_ships_lock().await;
        let mut shipyard_task_locs = BTreeSet::new();
        let mut purchased_ships = vec![];

        let mut failed_era = None;
        let probes = self.probed_waypoints();
        for job in self
            .ship_config
            .iter()
            .filter(|job| !self.job_assigned(&job.id))
        {
            // Make sure we've bought all ships for a specific era before moving on to the next
            if let Some(failed_era) = &failed_era {
                if &job.era > failed_era {
                    break;
                }
            }

            // if ship docked at shipyard + credits available, buy ship immediately
            // otherwise, register as a (potential) task
            let system = self.starting_system();
            let mut shipyards = self
                .universe
                .search_shipyards(&system, &job.ship_model)
                .await;
            shipyards.sort_by_key(|x| x.1);
            let (shipyard, cost) = match shipyards.first() {
                Some((shipyard, cost)) => (shipyard, cost),
                None => {
                    debug!("Not buying ship {}: no shipyards", job.ship_model);
                    failed_era = Some(job.era);
                    continue;
                }
            };
            let current_credits = self.credits();
            if current_credits < cost + 500000 {
                // @@ sort out this limit based on trading ships
                debug!("Not buying ship {}: low credits", job.ship_model);
                failed_era = Some(job.era);
                continue;
            }
            let ship_symbol: Option<String> = self
                .ships
                .iter()
                .find(|ship| {
                    let ship = ship.value().lock().unwrap();
                    if ship.nav.waypoint_symbol != *shipyard || ship.nav.status == InTransit {
                        return false;
                    }
                    let is_probe = probes.iter().any(|(s, _w)| s == &ship.symbol);
                    let is_purchaser = match &purchaser {
                        Some(purchaser) => ship.symbol == *purchaser,
                        None => false,
                    };
                    is_probe || is_purchaser
                })
                .map(|ship| ship.key().clone());
            let ship_controller = match &ship_symbol {
                Some(ship_symbol) => self.ship_controller(ship_symbol),
                None => {
                    debug!(
                        "Not buying ship {}: no ship at {}",
                        job.ship_model, shipyard
                    );
                    shipyard_task_locs.insert(shipyard.clone());
                    failed_era = Some(job.era);
                    continue;
                }
            };
            let bought_ship_symbol = self.buy_ship(shipyard, &job.ship_model).await;
            ship_controller.refresh_shipyard().await;
            let assigned = self.try_assign_ship(&bought_ship_symbol).await;
            assert!(assigned);
            purchased_ships.push(bought_ship_symbol);
        }
        (purchased_ships, shipyard_task_locs)
    }

    pub async fn run_ships(&self) {
        let self_clone = self.clone();
        {
            let join_hdl = tokio::spawn(async move {
                let broker = self_clone.siphon_cargo_broker.clone();
                broker.run(Box::new(self_clone)).await;
            });
            debug!("spawn_broker try push join_hdl");
            self.hdls.push(join_hdl).await;
            debug!("spawn_broker pushed join_hdl");
        }
        for ship in self.ships.iter() {
            let ship_symbol = ship.key().clone();
            if !self.ship_assigned(&ship_symbol) {
                self.try_assign_ship(&ship_symbol).await;
            }
        }
        let (_bought, _tasks) = self.try_buy_ships(None).await;
        dbg!(&self.job_assignments);
        dbg!(&self.job_assignments_rev);

        let self_clone = self.clone();
        let start = tokio::spawn(async move {
            for ship in self_clone.ships.iter() {
                let ship_symbol = ship.key().clone();
                self_clone.spawn_run_ship(ship_symbol).await;
            }
        });
        self.hdls.wait_all(Some(start)).await;
        info!("All ships have completed their tasks");
    }

    pub async fn try_assign_ship(&self, ship_symbol: &str) -> bool {
        assert!(!self.job_assignments_rev.contains_key(ship_symbol));
        let ship = self.ships.get(ship_symbol).unwrap();
        let ship_model = { ship.lock().unwrap().model().unwrap() };
        let job_opt = self.ship_config.iter().find(|job| {
            !self.job_assignments.contains_key(&job.id) && job.ship_model == ship_model
        });
        match job_opt {
            Some(job) => {
                self.job_assignments
                    .insert(job.id.clone(), ship_symbol.to_string());
                self.job_assignments_rev
                    .insert(ship_symbol.to_string(), job.id.clone());
                info!(
                    "Assigned {} ({}) to job {}",
                    ship_symbol, ship_model, job.id,
                );
                self.db
                    .set_value(
                        &format!("{}/ship_assignments", self.callsign),
                        self.job_assignments.deref(),
                    )
                    .await;
                true
            }
            None => {
                debug!(
                    "No job available for ship {} of model {}",
                    ship_symbol, ship_model
                );
                false
            }
        }
    }

    pub fn _spawn_run_ship(&self, ship_symbol: String) -> BoxFuture<()> {
        Box::pin(self.spawn_run_ship(ship_symbol))
    }

    pub async fn spawn_run_ship(&self, ship_symbol: String) {
        debug!("Spawning task for {}", ship_symbol);
        match self.job_assignments_rev.get(&ship_symbol) {
            Some(job_id) => {
                let job_spec = self
                    .ship_config
                    .iter()
                    .find(|s| s.id == *job_id)
                    .expect("job_id not found in ship_config_spec");
                // run script for assigned job
                let join_hdl = match &job_spec.behaviour {
                    ShipBehaviour::FixedProbe(waypoint_symbol) => {
                        let ship_controller = self.ship_controller(&ship_symbol);
                        let waypoint_symbol = waypoint_symbol.clone();
                        tokio::spawn(async move {
                            ship_scripts::probe::run(ship_controller, &waypoint_symbol).await;
                        })
                    }
                    ShipBehaviour::Logistics => {
                        let ship_controller = self.ship_controller(&ship_symbol);
                        let db = self.db.clone();
                        let task_manager = self.task_manager.clone();
                        tokio::spawn(async move {
                            ship_scripts::logistics::run(ship_controller, db, task_manager).await;
                        })
                    }
                    ShipBehaviour::SiphonDrone => {
                        let ship_controller = self.ship_controller(&ship_symbol);
                        tokio::spawn(async move {
                            ship_scripts::siphon::run_drone(ship_controller).await;
                        })
                    }
                    ShipBehaviour::SiphonShuttle => {
                        let ship_controller = self.ship_controller(&ship_symbol);
                        let db = self.db.clone();
                        tokio::spawn(async move {
                            ship_scripts::siphon::run_shuttle(ship_controller, db).await;
                        })
                    }
                    ShipBehaviour::MiningDrone => {
                        let ship_controller = self.ship_controller(&ship_symbol);
                        tokio::spawn(async move {
                            ship_scripts::mining::run_mining_drone(ship_controller).await;
                        })
                    }
                    ShipBehaviour::MiningShuttle => {
                        let ship_controller = self.ship_controller(&ship_symbol);
                        let db = self.db.clone();
                        tokio::spawn(async move {
                            ship_scripts::mining::run_shuttle(ship_controller, db).await;
                        })
                    }
                    ShipBehaviour::MiningSurveyor => {
                        let ship_controller = self.ship_controller(&ship_symbol);
                        tokio::spawn(async move {
                            ship_scripts::mining::run_surveyor(ship_controller).await;
                        })
                    }
                };
                debug!("spawn_run_ship try push join_hdl");
                self.hdls.push(join_hdl).await;
                // self.ship_futs.lock().unwrap().push_back(join_hdl);
                debug!("spawn_run_ship pushed join_hdl");
            }
            None => {
                debug!("Warning. No job assigned to ship {}", ship_symbol);
            }
        }
    }
}

use tokio::task::JoinHandle;

#[derive(Debug)]
struct JoinHandles {
    handles: Arc<Mutex<FuturesUnordered<JoinHandle<()>>>>,
    rx: Arc<Mutex<tokio::sync::mpsc::Receiver<JoinHandle<()>>>>,
    tx: tokio::sync::mpsc::Sender<JoinHandle<()>>,
}
impl JoinHandles {
    fn new() -> Self {
        let (tx, rx) = tokio::sync::mpsc::channel::<JoinHandle<()>>(1);
        Self {
            handles: Arc::new(Mutex::new(FuturesUnordered::new())),
            rx: Arc::new(Mutex::new(rx)),
            tx,
        }
    }
    async fn push(&self, handle: tokio::task::JoinHandle<()>) {
        self.tx.send(handle).await.unwrap();
    }
    async fn wait_all(&self, start: Option<tokio::task::JoinHandle<()>>) {
        use futures::StreamExt as _;
        let mut handles = self.handles.lock().unwrap();
        let mut rx = self.rx.lock().unwrap();

        if let Some(start) = start {
            debug!("JoinHandles::wait_all: adding new (start) handle");
            handles.push(start);
        }
        loop {
            tokio::select! {
                hdl_ret = handles.next() => {
                    hdl_ret.unwrap().unwrap();
                    debug!("JoinHandles::wait_all: handle completed");
                }
                handle = rx.recv() => {
                    debug!("JoinHandles::wait_all: adding new handle");
                    handles.push(handle.unwrap());
                }
            }
        }
    }
}
