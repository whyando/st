use crate::agent_controller::AgentController;
use crate::data::DataClient;
use crate::logistics_planner::plan::task_to_scheduled_action;
use crate::logistics_planner::{
    self, Action, LogisticShip, PlannerConstraints, ShipSchedule, Task, TaskActions,
};
use crate::models::MarketSupply::*;
use crate::models::MarketType::*;
use crate::models::*;
use crate::models::{LogisticsScriptConfig, MarketActivity::*};
use crate::universe::Universe;
use chrono::{DateTime, Duration, Utc};
use dashmap::DashMap;
use log::*;
use std::cmp::min;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, RwLock};

fn is_task_allowed(task: &Task, config: &LogisticsScriptConfig) -> bool {
    if let Some(waypoint_allowlist) = &config.waypoint_allowlist {
        match &task.actions {
            TaskActions::VisitLocation { waypoint, .. } => {
                if !waypoint_allowlist.contains(&waypoint) {
                    return false;
                }
            }
            TaskActions::TransportCargo { src, dest, .. } => {
                if !waypoint_allowlist.contains(&src) || !waypoint_allowlist.contains(&dest) {
                    return false;
                }
            }
        }
    }
    match &task.actions {
        TaskActions::VisitLocation { action, .. } => match action {
            Action::RefreshMarket => config.allow_market_refresh,
            Action::RefreshShipyard => config.allow_market_refresh,
            Action::TryBuyShips => config.allow_shipbuying,
            _ => true,
        },
        TaskActions::TransportCargo { dest_action, .. } => match dest_action {
            Action::DeliverConstruction(_, _) => config.allow_construction,
            _ => true,
        },
    }
}

#[derive(Clone)]
pub struct LogisticTaskManager {
    pub system_symbol: SystemSymbol,
    agent_controller: Arc<RwLock<Option<AgentController>>>,
    universe: Universe,
    db_client: DataClient,

    // task_id -> (task, ship_symbol, timestamp)
    in_progress_tasks: Arc<DashMap<String, (Task, String, DateTime<Utc>)>>,
    take_tasks_mutex_guard: Arc<tokio::sync::Mutex<()>>,
}

impl LogisticTaskManager {
    pub async fn new(
        universe: &Universe,
        db_client: &DataClient,
        system_symbol: &SystemSymbol,
    ) -> Self {
        let in_progress_tasks = db_client
            .load_task_manager_state(system_symbol)
            .await
            .unwrap_or_default();
        Self {
            system_symbol: system_symbol.clone(),
            universe: universe.clone(),
            db_client: db_client.clone(),
            agent_controller: Arc::new(RwLock::new(None)),
            in_progress_tasks: Arc::new(in_progress_tasks),
            take_tasks_mutex_guard: Arc::new(tokio::sync::Mutex::new(())),
        }
    }

    pub fn in_progress_tasks(&self) -> Arc<DashMap<String, (Task, String, DateTime<Utc>)>> {
        self.in_progress_tasks.clone()
    }

    pub fn get_assigned_task_status(&self, task_id: &str) -> Option<(Task, String, DateTime<Utc>)> {
        self.in_progress_tasks.get(task_id).map(|v| v.clone())
    }

    pub fn set_agent_controller(&self, ac: &AgentController) {
        let mut agent_controller = self.agent_controller.write().unwrap();
        assert!(agent_controller.is_none());
        *agent_controller = Some(ac.clone());
    }

    fn probe_locations(&self) -> Vec<WaypointSymbol> {
        self.agent_controller()
            .probed_waypoints()
            .into_iter()
            .flat_map(|w| w.1)
            .collect()
    }
    fn agent_controller(&self) -> AgentController {
        self.agent_controller
            .read()
            .unwrap()
            .as_ref()
            .unwrap()
            .clone()
    }

    // add trading tasks to the task list, if they don't already exist
    // (this function is not without side effects: it may buy ships)
    pub async fn generate_task_list(&self, capacity_cap: i64, buy_ships: bool) -> Vec<Task> {
        let now = chrono::Utc::now();
        let waypoints: Vec<Waypoint> = self
            .universe
            .get_system_waypoints(&self.system_symbol)
            .await;

        let mut tasks = Vec::new();

        // execute contract actions + generate tasks
        // (todo)

        // execute ship_buy actions + generate tasks
        let (bought, shipyard_task_waypoint) = match buy_ships {
            true => self.agent_controller().try_buy_ships(None).await,
            false => (Vec::new(), None),
        };
        info!(
            "Task Controller buy phase resulted in {} ships bought",
            bought.len()
        );
        for ship_symbol in bought {
            debug!("Task controller bought ship {}", ship_symbol);
            self.agent_controller()._spawn_run_ship(ship_symbol).await;
        }
        if let Some(waypoint) = shipyard_task_waypoint {
            tasks.push(Task {
                id: format!("buyships_{}", waypoint),
                actions: TaskActions::VisitLocation {
                    waypoint: waypoint.clone(),
                    action: Action::TryBuyShips,
                },
                value: 200000,
            });
        }

        // load markets
        let mut markets = Vec::new();
        let mut shipyards = Vec::new();
        for waypoint in &waypoints {
            if waypoint.is_market() {
                let market_remote = self.universe.get_market_remote(&waypoint.symbol).await;
                let market_opt = self.universe.get_market(&waypoint.symbol).await;
                markets.push((market_remote, market_opt));
            }
            if waypoint.is_shipyard() {
                let shipyard_remote = self.universe.get_shipyard_remote(&waypoint.symbol).await;
                let shipyard_opt = self.universe.get_shipyard(&waypoint.symbol).await;
                shipyards.push((shipyard_remote, shipyard_opt));
            }
        }
        // unique list of goods
        let mut goods = BTreeSet::new();
        for (_, market_opt) in &markets {
            if let Some(market) = market_opt {
                for good in &market.data.trade_goods {
                    goods.insert(good.symbol.clone());
                }
            }
        }

        // Construction tasks
        let jump_gate = waypoints
            .iter()
            .find(|w| w.is_jump_gate())
            .expect("Star system has no jump gate");
        let mut blacklist_trade_goods = BTreeSet::new();
        let construction = self.universe.get_construction(&jump_gate.symbol).await;
        if let Some(construction) = &construction.data {
            for material in &construction.materials {
                if material.fulfilled >= material.required {
                    continue;
                }
                // Don't trade goods for profit if we need them for construction
                blacklist_trade_goods.insert(material.trade_symbol.clone());

                let remaining = material.required - material.fulfilled;
                let buy_trade_good = markets
                    .iter()
                    .filter_map(|(_, market_opt)| match market_opt {
                        Some(market) => {
                            let market_symbol = market.data.symbol.clone();
                            let trade = market
                                .data
                                .trade_goods
                                .iter()
                                .find(|g| g.symbol == material.trade_symbol);
                            trade.map(|trade| (market_symbol, trade))
                        }
                        None => None,
                    })
                    // purchase filters
                    .filter(|(_, trade)| match trade._type {
                        Import => false,
                        Export => {
                            // unsure if this is just causing weird fluctuations
                            // Strong markets are where we'll make the most consistent profit
                            // ?? what about RESTRICTED markets?
                            if trade.activity == Some(Strong) {
                                trade.supply >= High
                            } else {
                                trade.supply >= Moderate
                            }
                            // trade.supply >= Moderate
                        }
                        Exchange => true,
                    })
                    .min_by_key(|(_, trade)| trade.purchase_price);
                if let Some(buy_trade_good) = buy_trade_good {
                    let units = min(min(remaining, capacity_cap), buy_trade_good.1.trade_volume);
                    let cost = units * buy_trade_good.1.purchase_price;
                    // if cost + 2000000 <= available_credits {
                    debug!(
                        "Construction: buy {} @ {} for ${}, progress: {}/{}",
                        material.trade_symbol,
                        buy_trade_good.1.purchase_price,
                        cost,
                        material.fulfilled,
                        material.required
                    );
                    tasks.push(Task {
                        id: format!("construction_{}", material.trade_symbol),
                        actions: TaskActions::TransportCargo {
                            src: buy_trade_good.0.clone(),
                            dest: jump_gate.symbol.clone(),
                            src_action: Action::BuyGoods(material.trade_symbol.clone(), units),
                            dest_action: Action::DeliverConstruction(
                                material.trade_symbol.clone(),
                                units,
                            ),
                        },
                        value: 100000,
                    });
                    // }
                }
            }
        }

        let probe_locations = self.probe_locations();
        for (market_remote, market_opt) in &markets {
            let requires_visit = match market_opt {
                Some(market) => {
                    now.signed_duration_since(market.timestamp) >= Duration::try_hours(1).unwrap()
                }
                None => true,
            };
            let is_probed = probe_locations.contains(&market_remote.symbol);
            // Some fuel stop markets only trade fuel, so not worth visiting
            let is_pure_exchange =
                market_remote.exports.is_empty() && market_remote.imports.is_empty();
            if requires_visit && !is_pure_exchange && !is_probed {
                tasks.push(Task {
                    id: format!("refreshmarket_{}", market_remote.symbol),
                    actions: TaskActions::VisitLocation {
                        waypoint: market_remote.symbol.clone(),
                        action: Action::RefreshMarket,
                    },
                    value: 20000,
                });
            }
        }
        for (shipyard_remote, shipyard_opt) in &shipyards {
            let requires_visit = match shipyard_opt {
                Some(_shipyard) => false,
                None => true,
            };
            let is_probed = probe_locations.contains(&shipyard_remote.symbol);
            if requires_visit && !is_probed {
                tasks.push(Task {
                    id: format!("refreshshipyard_{}", shipyard_remote.symbol),
                    actions: TaskActions::VisitLocation {
                        waypoint: shipyard_remote.symbol.clone(),
                        action: Action::RefreshShipyard,
                    },
                    value: 5000,
                });
            }
        }

        for good in goods {
            if blacklist_trade_goods.contains(&good) {
                continue;
            }
            let trades = markets
                .iter()
                .filter_map(|(_, market_opt)| match market_opt {
                    Some(market) => {
                        let market_symbol = market.data.symbol.clone();
                        let trade = market.data.trade_goods.iter().find(|g| g.symbol == good);
                        trade.map(|trade| (market_symbol, trade))
                    }
                    None => None,
                })
                .collect::<Vec<_>>();
            let buy_trade_good = trades
                .iter()
                .filter(|(_, trade)| match trade._type {
                    Import => false,
                    Export => {
                        // !! unsure if this is just causing weird fluctuations
                        // Strong markets are where we'll make the most consistent profit
                        // ?? what about RESTRICTED markets?
                        if trade.activity == Some(Strong) {
                            trade.supply >= High
                        } else {
                            trade.supply >= Moderate
                        }
                        // trade.supply >= Moderate
                    }
                    Exchange => true,
                })
                .min_by_key(|(_, trade)| trade.purchase_price);
            let sell_trade_good = trades
                .iter()
                .filter(|(_, trade)| match trade._type {
                    Import => trade.supply <= Moderate,
                    Export => false,
                    Exchange => true,
                })
                .max_by_key(|(_, trade)| trade.sell_price);
            let (buy_trade_good, sell_trade_good) = match (buy_trade_good, sell_trade_good) {
                (Some(buy), Some(sell)) => (buy, sell),
                _ => continue,
            };
            let units = min(
                min(
                    buy_trade_good.1.trade_volume,
                    sell_trade_good.1.trade_volume,
                ),
                capacity_cap,
            );
            let profit =
                (sell_trade_good.1.sell_price - buy_trade_good.1.purchase_price) * (units as i64);
            let can_afford = true; // logistic ships reserve their credits beforehand
            if profit > 0 && can_afford {
                debug!(
                    "{}: buy {} @ {} for ${}, sell @ {} for ${}, profit: ${}",
                    good,
                    units,
                    buy_trade_good.0,
                    buy_trade_good.1.purchase_price,
                    sell_trade_good.0,
                    sell_trade_good.1.sell_price,
                    profit
                );
                tasks.push(Task {
                    // exclusion seems a bit broad right now, but it's a start
                    id: format!("trade_{}", good),
                    actions: TaskActions::TransportCargo {
                        src: buy_trade_good.0.clone(),
                        dest: sell_trade_good.0.clone(),
                        src_action: Action::BuyGoods(good.clone(), units),
                        dest_action: Action::SellGoods(good.clone(), units),
                    },
                    value: profit,
                });
            }
        }
        tasks
    }

    async fn take_tasks_lock(&self) -> tokio::sync::MutexGuard<()> {
        match self.take_tasks_mutex_guard.try_lock() {
            Ok(guard) => guard,
            Err(_e) => {
                debug!("LogisticTaskManager::take_tasks is already running");
                let timeout = tokio::time::Duration::from_secs(30);
                match tokio::time::timeout(timeout, self.take_tasks_mutex_guard.lock()).await {
                    Ok(guard) => {
                        debug!("LogisticTaskManager::take_tasks lock acquired");
                        guard
                    }
                    Err(_e) => {
                        panic!("LogisticTaskManager::take_tasks lock timeout");
                    }
                }
            }
        }
    }

    // Provide a set of tasks for a single ship
    pub async fn take_tasks(
        &self,
        ship_symbol: &str,
        config: &LogisticsScriptConfig,
        cargo_capacity: i64,
        engine_speed: i64,
        fuel_capacity: i64,
        start_waypoint: &WaypointSymbol,
        plan_length: Duration,
    ) -> ShipSchedule {
        let _guard = self.take_tasks_lock().await;
        assert_eq!(start_waypoint.system(), self.system_symbol);

        // Cleanup in_progress_tasks for this ship
        self.in_progress_tasks.retain(|_k, v| v.1 != ship_symbol);
        let all_tasks = self.generate_task_list(cargo_capacity, true).await;
        self.agent_controller()
            .ledger
            .reserve_credits(ship_symbol, 5000 * cargo_capacity);

        // Filter out tasks that are already in progress
        // Also filter tasks outlawed by the config for this ship
        let available_tasks = all_tasks
            .into_iter()
            .filter(|task| !self.in_progress_tasks.contains_key(&task.id))
            .filter(|task| is_task_allowed(&task, config))
            .collect::<Vec<_>>();

        let matrix = self
            .universe
            .estimate_duration_matrix(&self.system_symbol, engine_speed, fuel_capacity)
            .await;
        let logistics_ship = LogisticShip {
            symbol: ship_symbol.to_string(),
            capacity: cargo_capacity,
            speed: engine_speed,
            start_waypoint: start_waypoint.clone(),
            // available_from: Duration::seconds(0), // if we need to account for in-progress task(s)
        };
        let contraints = PlannerConstraints {
            plan_length,
            max_compute_time: Duration::try_seconds(5).unwrap(),
        };
        let available_tasks_clone = available_tasks.clone();
        let (mut task_assignments, schedules) = if config.use_planner {
            tokio::task::spawn_blocking(move || {
                logistics_planner::plan::run_planner(
                    &[logistics_ship],
                    &available_tasks_clone,
                    &matrix,
                    &contraints,
                )
            })
            .await
            .unwrap()
        } else {
            let ship_schedule = ShipSchedule {
                ship: logistics_ship,
                actions: vec![],
            };
            (BTreeMap::new(), vec![ship_schedule])
        };
        assert_eq!(schedules.len(), 1);
        let mut schedule = schedules.into_iter().next().unwrap();

        // If 0 tasks were assigned, instead force assign the highest value task
        if schedule.actions.len() == 0 {
            let mut highest_value_task = None;
            let mut highest_value = 0;
            for task in available_tasks {
                if task.value > highest_value {
                    highest_value = task.value;
                    highest_value_task = Some(task);
                }
            }
            if let Some(task) = highest_value_task {
                info!(
                    "Forcing assignment of task {} value: {}",
                    task.id, task.value
                );
                // add actions for the task
                match &task.actions {
                    TaskActions::VisitLocation { .. } => {
                        schedule
                            .actions
                            .push(task_to_scheduled_action(&task, "", None));
                    }
                    TaskActions::TransportCargo { .. } => {
                        schedule
                            .actions
                            .push(task_to_scheduled_action(&task, "pickup", None));
                        schedule
                            .actions
                            .push(task_to_scheduled_action(&task, "delivery", None));
                    }
                };
                task_assignments.insert(task, Some(ship_symbol.to_string()));
            }
        }

        for (task, ship) in task_assignments {
            if let Some(ship) = &ship {
                debug!("Assigned task {} to ship {}", task.id, ship);
                self.in_progress_tasks
                    .insert(task.id.clone(), (task.clone(), ship.clone(), Utc::now()));
            }
        }
        self.db_client
            .save_task_manager_state(&self.system_symbol, &self.in_progress_tasks)
            .await;

        schedule
    }

    pub async fn set_task_completed(&self, task: &Task) {
        self.in_progress_tasks.remove(&task.id);
        self.db_client
            .save_task_manager_state(&self.system_symbol, &self.in_progress_tasks)
            .await;
        debug!("Marking task {} as completed", task.id);
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn test_logistic_task_manager_state() {
        let in_progress_tasks = DashMap::<String, (Task, String, DateTime<Utc>)>::new();
        let task = Task {
            id: "test".to_string(),
            actions: TaskActions::VisitLocation {
                waypoint: WaypointSymbol("A".to_string()),
                action: Action::RefreshMarket,
            },
            value: 20000,
        };
        in_progress_tasks.insert(
            "test".to_string(),
            (task.clone(), "ship".to_string(), Utc::now()),
        );
        let _json = serde_json::to_string(&in_progress_tasks).unwrap();
    }
}
