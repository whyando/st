use super::ledger::Ledger;
use crate::broker::{CargoBroker, TransferActor};
use crate::config::CONFIG;
use crate::models::{ShipNavStatus::*, *};
use crate::ship_config::{ship_config_capital_system, ship_config_starter_system};
use crate::survey_manager::SurveyManager;
use crate::universe::WaypointFilter;
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
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::ops::Deref;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc::Sender;

#[derive(Clone, Debug)]
pub enum Event {
    ShipUpdate(Ship),
    AgentUpdate(Agent),
}

#[derive(Clone, Debug)]
enum BuyShipResult {
    Bought(String),
    FailedNeverPurchase,
    FailedLowCredits,
    FailedNoShipyards,
    // if we failed because there was no purchaser available,
    // we can return a waypoint symbol to indicate a task should be created
    // to go there
    FailedNoPurchaser(Option<WaypointSymbol>),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum AgentEra {
    // Initial era, where the agent has two ships
    StartingSystem1,

    // Some credit threshold has been met: buy more ships
    StartingSystem2,

    // Jumpgate is completed, agent has access to the capital system
    InterSystem1,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct AgentState {
    pub era: AgentEra,
}

impl Default for AgentState {
    fn default() -> Self {
        Self {
            era: AgentEra::StartingSystem1,
        }
    }
}

#[derive(Clone)]
pub struct AgentController {
    universe: Universe,
    api_client: ApiClient,
    db: DataClient,

    listeners: Arc<Mutex<Vec<Sender<Event>>>>,
    callsign: String,
    state: Arc<Mutex<AgentState>>,
    agent: Arc<Mutex<Agent>>,
    ships: Arc<DashMap<String, Arc<Mutex<Ship>>>>,

    ship_config: Arc<Mutex<Vec<ShipConfig>>>,
    job_assignments: Arc<DashMap<String, String>>,
    job_assignments_rev: Arc<DashMap<String, String>>,
    // ship_futs: Arc<Mutex<VecDeque<tokio::task::JoinHandle<()>>>>,
    hdls: Arc<JoinHandles>,
    pub task_manager: Arc<LogisticTaskManager>,
    pub survey_manager: Arc<SurveyManager>,
    pub cargo_broker: Arc<CargoBroker>,
    pub ledger: Arc<Ledger>,

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
    pub fn agent(&self) -> Agent {
        self.agent.lock().unwrap().clone()
    }
    pub fn state(&self) -> AgentState {
        self.state.lock().unwrap().clone()
    }
    pub fn ships(&self) -> Vec<Ship> {
        self.ships
            .iter()
            .map(|x| x.value().lock().unwrap().clone())
            .collect()
    }

    pub fn add_event_listener(&self, listener: Sender<Event>) {
        let mut listeners = self.listeners.lock().unwrap();
        listeners.push(listener);
        info!("Added event listener");
        // web api should only require one listener, although we could support multiple
        assert!(listeners.len() <= 1);
    }

    // definitely causing issues
    // pub fn emit_event_blocking(&self, event: &Event) {
    //     let listeners = { self.listeners.lock().unwrap().clone() };
    //     for listener in listeners.iter() {
    //         listener.blocking_send(event.clone()).unwrap();
    //     }
    // }
    pub async fn emit_event(&self, event: &Event) {
        let listeners = { self.listeners.lock().unwrap().clone() };
        for listener in listeners.iter() {
            listener.send(event.clone()).await.unwrap();
        }
    }

    pub async fn transfer_cargo(
        &self,
        src_ship_symbol: String,
        dest_ship_symbol: String,
        good: String,
        units: i64,
    ) {
        debug!("agent_controller::transfer_cargo");

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
        let (src_ship, dest_ship) = {
            let src_ship = self.ships.get(&src_ship_symbol).unwrap();
            let dest_ship = self.ships.get(&dest_ship_symbol).unwrap();
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
            (src_ship.clone(), dest_ship.clone())
        };
        self.emit_event(&Event::ShipUpdate(src_ship)).await;
        self.emit_event(&Event::ShipUpdate(dest_ship)).await;
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
        let task_manager = LogisticTaskManager::new(universe, db, &system_symbol).await;
        let survey_manager = SurveyManager::new(db).await;

        let initial_credits = {
            let agent = agent.lock().unwrap();
            agent.credits
        };
        let ledger = Ledger::new(initial_credits);
        let state: AgentState = db
            .get_value(&format!("{}/state", callsign))
            .await
            .unwrap_or_default();
        let agent_controller = Self {
            callsign: callsign.to_string(),
            state: Arc::new(Mutex::new(state)),
            agent,
            ships,
            api_client: api_client.clone(),
            db: db.clone(),
            universe: universe.clone(),
            listeners: Arc::new(Mutex::new(Vec::new())),
            // ship_futs: Arc::new(Mutex::new(VecDeque::new())),
            hdls: Arc::new(JoinHandles::new()),
            ship_config: Arc::new(Mutex::new(vec![])),
            job_assignments: Arc::new(job_assignments),
            job_assignments_rev: Arc::new(job_assignments_rev),
            task_manager: Arc::new(task_manager),
            cargo_broker: Arc::new(CargoBroker::new()),
            survey_manager: Arc::new(survey_manager),
            try_buy_ships_mutex_guard: Arc::new(tokio::sync::Mutex::new(())),
            ledger: Arc::new(ledger),
        };
        agent_controller
            .task_manager
            .set_agent_controller(&agent_controller);
        let credits = agent_controller.ledger.credits();
        let num_ships = agent_controller.num_ships();
        info!(
            "Loaded agent {} ${} with {} ships",
            callsign, credits, num_ships
        );
        info!(
            "{} effective reserved credits, {} available",
            agent_controller.ledger.effective_reserved_credits(),
            agent_controller.ledger.available_credits()
        );
        agent_controller
    }
    // pub fn credits(&self) -> i64 {
    //     self.agent.lock().unwrap().credits
    // }
    pub fn starting_system(&self) -> SystemSymbol {
        self.agent.lock().unwrap().headquarters.system()
    }
    pub fn starting_faction(&self) -> String {
        self.agent.lock().unwrap().starting_faction.clone()
    }
    pub fn num_ships(&self) -> usize {
        self.ships.len()
    }
    pub fn get_ship_config(&self) -> Vec<ShipConfig> {
        self.ship_config.lock().unwrap().clone()
    }
    pub fn set_ship_config(&self, config: Vec<ShipConfig>) {
        let mut ship_config = self.ship_config.lock().unwrap();
        *ship_config = config;
    }
    pub async fn update_agent(&self, agent_upd: Agent) {
        self.emit_event(&Event::AgentUpdate(agent_upd.clone()))
            .await;
        let mut agent = self.agent.lock().unwrap();
        *agent = agent_upd;
        self.ledger.set_credits(agent.credits);
    }
    fn debug(&self, msg: &str) {
        debug!("[{}] {}", self.callsign, msg);
    }
    pub async fn faction_capital(&self) -> SystemSymbol {
        let faction_symbol = self.starting_faction();
        let faction = self.universe.get_faction(&faction_symbol).await;
        faction.headquarters.unwrap()
    }
    pub async fn update_era(&self, era: AgentEra) {
        let state = {
            let mut state = self.state.lock().unwrap();
            state.era = era;
            state.clone()
        };
        self.db
            .set_value(&format!("{}/state", self.callsign), &state)
            .await;
    }

    pub async fn check_era_advance(&self) {
        loop {
            let current_era = self.state().era;
            let next_era = match current_era {
                AgentEra::StartingSystem1 => {
                    // Conditions for going to mid:
                    // - 1000000 credits available
                    let credits = self.ledger.available_credits();
                    if credits >= 1_000_000 {
                        Some(AgentEra::StartingSystem2)
                    } else {
                        None
                    }
                }
                AgentEra::StartingSystem2 => {
                    let jumpgate_finished = self.is_jumpgate_finished().await;
                    if jumpgate_finished {
                        Some(AgentEra::InterSystem1)
                    } else {
                        None
                    }
                }
                AgentEra::InterSystem1 => None,
            };
            match next_era {
                None => break,
                Some(next_era) => {
                    assert_ne!(current_era, next_era);
                    info!("Agent {} advancing to era {:?}", self.callsign, next_era);
                    self.update_era(next_era).await;
                }
            }
        }
    }

    pub fn probed_waypoints(&self) -> Vec<(String, Vec<WaypointSymbol>)> {
        let ship_config = self.ship_config.lock().unwrap();
        ship_config
            .iter()
            .filter_map(|job| {
                if let ShipBehaviour::Probe(config) = &job.behaviour {
                    if let Some(assignment) = self.job_assignments.get(&job.id) {
                        let ship_symbol = assignment.value().clone();
                        return Some((ship_symbol, config.waypoints.clone()));
                    }
                }
                None
            })
            .collect()
    }

    // Waypoints that are probed, and the probe never leaves that single waypoint
    pub fn statically_probed_waypoints(&self) -> Vec<(String, WaypointSymbol)> {
        let ship_config = self.ship_config.lock().unwrap();
        ship_config
            .iter()
            .filter_map(|job| {
                if let ShipBehaviour::Probe(config) = &job.behaviour {
                    let waypoints = &config.waypoints;
                    if waypoints.len() != 1 {
                        return None;
                    }
                    let waypoint_symbol = &waypoints[0];
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
        self.update_agent(agent).await;
        self.ships
            .insert(ship_symbol.clone(), Arc::new(Mutex::new(ship)));
        ship_symbol
    }

    pub fn ship_controller(&self, ship_symbol: &str) -> ShipController {
        let ship = self.ships.get(ship_symbol).unwrap();
        ShipController::new(&self.api_client, &self.universe, ship.clone(), self)
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

    // An attempt to buy a single specific ship
    async fn try_buy_ship(&self, purchaser: &Option<String>, job: &ShipConfig) -> BuyShipResult {
        let purchase_criteria = &job.purchase_criteria;
        debug!(
            "try_buy_ship ({:?}): {} {} {:?}",
            purchaser, job.id, job.ship_model, purchase_criteria
        );
        if purchase_criteria.never_purchase {
            return BuyShipResult::FailedNeverPurchase;
        }
        let purchase_system = match &purchase_criteria.system_symbol {
            Some(system_symbol) => system_symbol.clone(),
            None => self.starting_system(),
        };

        // if ship docked at shipyard + credits available, buy ship immediately
        // otherwise, register as a (potential) task
        let mut shipyards = self
            .universe
            .search_shipyards(&purchase_system, &job.ship_model)
            .await;
        shipyards.sort_by_key(|x| x.1);

        if shipyards.len() == 0 {
            return BuyShipResult::FailedNoShipyards;
        }
        let job_credit_reservation = match &job.behaviour {
            ShipBehaviour::Logistics(_) => {
                SHIP_MODELS[job.ship_model.as_str()].cargo_capacity * 5000
            }
            _ => 0,
        };
        let current_credits = self.ledger.available_credits();
        let cheapest_shipard = shipyards[0].0.clone();
        let can_afford_cheapest = current_credits >= shipyards[0].1 + job_credit_reservation;
        debug!("try_buy_ship Credits available: {}", current_credits);
        debug!(
            "try_buy_ship Extra credits for job reservation: {}",
            job_credit_reservation
        );

        let static_probes = self.statically_probed_waypoints();
        for (shipyard, cost) in &shipyards {
            if current_credits < cost + job_credit_reservation {
                break; // no point looking at more expensive shipyards
            }
            // look for a purchaser
            let ship_symbol: Option<String> = self
                .ships
                .iter()
                .find(|ship| {
                    let ship = ship.value().lock().unwrap();
                    if ship.nav.waypoint_symbol != *shipyard || ship.nav.status == InTransit {
                        return false;
                    }
                    let is_static_probe = static_probes.iter().any(|(s, _w)| s == &ship.symbol);
                    let is_purchaser = match &purchaser {
                        Some(purchaser) => ship.symbol == *purchaser,
                        None => false,
                    };
                    is_static_probe || is_purchaser
                })
                .map(|ship| ship.key().clone());
            let ship_controller = match &ship_symbol {
                Some(ship_symbol) => self.ship_controller(ship_symbol),
                None => {
                    // this 'no purchaser' case is the only one where we iterate through the other shipyards
                    if purchase_criteria.require_cheapest {
                        break;
                    } else {
                        continue;
                    }
                }
            };
            let bought_ship_symbol = self.buy_ship(shipyard, &job.ship_model).await;
            ship_controller.refresh_shipyard().await;
            let assigned = self.try_assign_ship(&bought_ship_symbol).await;
            assert!(assigned);
            return BuyShipResult::Bought(bought_ship_symbol);
        }
        if !can_afford_cheapest {
            return BuyShipResult::FailedLowCredits;
        }
        if purchase_criteria.allow_logistic_task {
            BuyShipResult::FailedNoPurchaser(Some(cheapest_shipard))
        } else {
            BuyShipResult::FailedNoPurchaser(None)
        }
    }

    pub async fn try_buy_ships(
        &self,
        purchaser: Option<String>,
    ) -> (Vec<String>, Option<WaypointSymbol>) {
        let _guard = self.try_buy_ships_lock().await;

        self.check_era_advance().await;
        self.refresh_ship_config().await;

        if CONFIG.scrap_all_ships {
            return (vec![], None);
        }

        let mut purchased_ships = vec![];

        let ship_config = self.get_ship_config();
        for job in ship_config.iter().filter(|job| !self.job_assigned(&job.id)) {
            let result = self.try_buy_ship(&purchaser, &job).await;
            match result {
                BuyShipResult::Bought(ship_symbol) => {
                    purchased_ships.push(ship_symbol);
                }
                BuyShipResult::FailedNeverPurchase => {
                    debug!("Not buying ship {}: never_purchase", job.ship_model);
                    return (purchased_ships, None);
                }
                BuyShipResult::FailedLowCredits => {
                    debug!("Not buying ship {}: low credits", job.ship_model);
                    return (purchased_ships, None);
                }
                BuyShipResult::FailedNoShipyards => {
                    debug!("Not buying ship {}: no shipyards", job.ship_model);
                    return (purchased_ships, None);
                }
                BuyShipResult::FailedNoPurchaser(waypoint) => {
                    if let Some(waypoint) = waypoint {
                        debug!(
                            "Not buying ship {}: no purchaser. Adding task @ {}",
                            job.ship_model, waypoint
                        );
                        return (purchased_ships, Some(waypoint));
                    }
                    debug!("Not buying ship {}: no purchaser", job.ship_model);
                    return (purchased_ships, None);
                }
            }
        }
        (purchased_ships, None)
    }

    pub fn reserve_credits_for_job(&self, job: &ShipConfig, ship_symbol: &str) {
        // Only reserve credits for logistics jobs
        match &job.behaviour {
            ShipBehaviour::Logistics(_) => {}
            _ => return,
        }
        let ship = self.ships.get(ship_symbol).unwrap();
        let ship = ship.lock().unwrap();
        self.ledger
            .reserve_credits(ship_symbol, ship.cargo.capacity * 5000);
    }

    pub async fn generate_ship_config(&self) -> Vec<ShipConfig> {
        let era = self.state().era;
        let start_system = self.starting_system();
        let waypoints: Vec<Waypoint> = self.universe.get_system_waypoints(&start_system).await;
        let markets = self.universe.get_system_markets_remote(&start_system).await;
        let shipyards = self
            .universe
            .get_system_shipyards_remote(&start_system)
            .await;

        let mut ships = vec![];

        let use_nonstatic_probes = true;
        let incl_outer_probes_and_siphons = match era {
            AgentEra::StartingSystem1 => false,
            _ => true,
        };
        ships.append(&mut ship_config_starter_system(
            &waypoints,
            &markets,
            &shipyards,
            use_nonstatic_probes,
            incl_outer_probes_and_siphons,
        ));

        if era == AgentEra::InterSystem1 {
            let capital = self.faction_capital().await;
            let waypoints: Vec<Waypoint> = self.universe.get_system_waypoints(&capital).await;
            let markets = self.universe.get_system_markets_remote(&capital).await;
            let shipyards = self.universe.get_system_shipyards_remote(&capital).await;
            ships.append(&mut ship_config_capital_system(
                &capital,
                &start_system,
                &waypoints,
                &markets,
                &shipyards,
                false,
            ));
        }
        ships
    }

    pub async fn is_jumpgate_finished(&self) -> bool {
        let jump_gate_symbol = {
            let waypoints = self
                .universe
                .search_waypoints(&self.starting_system(), &vec![WaypointFilter::JumpGate])
                .await;
            assert!(waypoints.len() == 1);
            waypoints[0].symbol.clone()
        };
        let construction = self.universe.get_construction(&jump_gate_symbol).await;
        match &construction.data {
            None => true,
            Some(x) => x.is_complete,
        }
    }

    pub async fn refresh_ship_config(&self) {
        let ship_config = self.generate_ship_config().await;
        self.set_ship_config(ship_config.clone());

        // Unassign
        let mut keys_to_remove = Vec::new();
        for it in self.job_assignments.iter() {
            let (job_id, ship_symbol) = it.pair();
            let job_exists = ship_config.iter().any(|job| job.id == *job_id);
            let ship_exists = self.ships.contains_key(ship_symbol);
            if !job_exists {
                // if the job no longer exists, unassign the ship,
                // May be risky because we don't know if the ship is in the middle of a task
                warn!(
                    "Unassigning ship {} from non-existant job {}",
                    ship_symbol, job_id
                );
                keys_to_remove.push((job_id.clone(), ship_symbol.clone()));
            }
            if !ship_exists {
                // if the ship no longer exists, unassign the job
                warn!(
                    "Unassigning non-existant ship {} from job {}",
                    ship_symbol, job_id
                );
                keys_to_remove.push((job_id.clone(), ship_symbol.clone()));
            }
        }
        for (job_id, ship_symbol) in keys_to_remove {
            self.job_assignments.remove(&job_id);
            self.job_assignments_rev.remove(&ship_symbol);
        }
        self.db
            .set_value(
                &format!("{}/ship_assignments", self.callsign),
                self.job_assignments.deref(),
            )
            .await;

        // Assign
        for ship in self.ships.iter() {
            let ship_symbol = ship.key().clone();
            if !self.ship_assigned(&ship_symbol) {
                self.try_assign_ship(&ship_symbol).await;
            }
        }

        // load/refresh ledger - important to do this before starting ship scripts or buying more ships
        self.ledger.reserve_credits("FUEL", 10000);
        for ship_config in ship_config {
            if let Some(ship_symbol) = &self.job_assignments.get(&ship_config.id) {
                let ship_symbol: &String = ship_symbol.value();
                self.reserve_credits_for_job(&ship_config, ship_symbol);
            }
        }
    }

    pub async fn run_ships(&self) {
        let self_clone = self.clone();
        {
            let join_hdl = tokio::spawn(async move {
                let broker = self_clone.cargo_broker.clone();
                broker.run(Box::new(self_clone)).await;
            });
            debug!("spawn_broker try push join_hdl");
            self.hdls.push(join_hdl).await;
            debug!("spawn_broker pushed join_hdl");
        }

        // Generate ship config, purchase + assign ships
        // purchased ships are assigned, but not yet started
        let (_bought, _tasks) = self.try_buy_ships(None).await;

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
        let ship_config = self.get_ship_config();
        let job_opt = ship_config.iter().find(|job| {
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
                self.reserve_credits_for_job(job, ship_symbol);
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

        if CONFIG.scrap_all_ships {
            let ship_controller = self.ship_controller(&ship_symbol);
            let join_hdl = tokio::spawn(async move {
                ship_scripts::scrap::run(ship_controller).await;
            });
            self.hdls.push(join_hdl).await;
            return;
        }

        match self.job_assignments_rev.get(&ship_symbol) {
            Some(job_id) => {
                let ship_config = self.get_ship_config();
                let job_spec = ship_config
                    .iter()
                    .find(|s| s.id == *job_id)
                    .unwrap_or_else(|| panic!("No job found for {}", *job_id));
                if !CONFIG.job_id_filter.is_match(&job_spec.id) {
                    return;
                }
                let ship_controller = self.ship_controller(&ship_symbol);
                let ship = ship_controller.ship();
                if ship.engine.condition.unwrap() < 0.0 {
                    warn!(
                        "Ship {} has engine condition {}",
                        ship_symbol,
                        ship.engine.condition.unwrap()
                    );
                    return;
                }
                if ship.frame.condition.unwrap() < 0.0 {
                    warn!(
                        "Ship {} has frame condition {}",
                        ship_symbol,
                        ship.frame.condition.unwrap()
                    );
                    return;
                }
                if ship.reactor.condition.unwrap() < 0.0 {
                    warn!(
                        "Ship {} has reactor condition {}",
                        ship_symbol,
                        ship.reactor.condition.unwrap()
                    );
                    return;
                }

                // run script for assigned job
                let join_hdl = match &job_spec.behaviour {
                    ShipBehaviour::Probe(config) => {
                        let config = config.clone();
                        tokio::spawn(async move {
                            ship_scripts::probe::run(ship_controller, &config).await;
                        })
                    }
                    ShipBehaviour::Logistics(config) => {
                        let db = self.db.clone();
                        let task_manager = self.task_manager.clone();
                        let config = config.clone();
                        tokio::spawn(async move {
                            ship_scripts::logistics::run(ship_controller, db, task_manager, config)
                                .await;
                        })
                    }
                    ShipBehaviour::SiphonDrone => tokio::spawn(async move {
                        ship_scripts::siphon::run_drone(ship_controller).await;
                    }),
                    ShipBehaviour::SiphonShuttle => {
                        let db = self.db.clone();
                        tokio::spawn(async move {
                            ship_scripts::siphon::run_shuttle(ship_controller, db).await;
                        })
                    }
                    ShipBehaviour::MiningDrone => tokio::spawn(async move {
                        ship_scripts::mining::run_mining_drone(ship_controller).await;
                    }),
                    ShipBehaviour::MiningShuttle => {
                        let db = self.db.clone();
                        tokio::spawn(async move {
                            ship_scripts::mining::run_shuttle(ship_controller, db).await;
                        })
                    }
                    ShipBehaviour::MiningSurveyor => tokio::spawn(async move {
                        ship_scripts::mining::run_surveyor(ship_controller).await;
                    }),
                    ShipBehaviour::ConstructionHauler => {
                        let db = self.db.clone();
                        tokio::spawn(async move {
                            ship_scripts::construction::run_hauler(ship_controller, db).await;
                        })
                    }
                    ShipBehaviour::Explorer => {
                        let db = self.db.clone();
                        tokio::spawn(async move {
                            ship_scripts::exploration::run_explorer(ship_controller, db).await;
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

// ! todo: replace JoinHandles with TaskTracker from tokio-util (or tokio::task::join_set::JoinSet also from tokio-util)
// https://docs.rs/tokio-util/0.7.10/tokio_util/task/task_tracker/struct.TaskTracker.html
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
