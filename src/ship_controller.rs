use crate::agent_controller::Event;
use crate::api_client::api_models::{NavigateResponse, OrbitResponse, TradeResponse};
use crate::models::{ShipCargoItem, ShipCooldown, Survey};
use crate::ship_controller::ShipNavStatus::*;
use crate::{
    agent_controller::AgentController, api_client::ApiClient, logistics_planner::Action, models::*,
    universe::Universe,
};
use log::*;
use reqwest::{Method, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::cmp::min;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct ShipController {
    pub ship_symbol: String,
    ship: Arc<Mutex<Ship>>,

    api_client: ApiClient,
    pub universe: Arc<Universe>,
    pub agent_controller: AgentController,
}

impl ShipController {
    pub fn new(
        api_client: &ApiClient,
        universe: &Arc<Universe>,
        ship: Arc<Mutex<Ship>>,
        agent_controller: &AgentController,
    ) -> ShipController {
        let symbol = ship.lock().unwrap().symbol.clone();
        ShipController {
            api_client: api_client.clone(),
            universe: universe.clone(),
            ship,
            ship_symbol: symbol,
            agent_controller: agent_controller.clone(),
        }
    }
    pub fn ship(&self) -> Ship {
        self.ship.lock().unwrap().clone()
    }
    pub fn symbol(&self) -> String {
        self.ship_symbol.clone()
    }
    pub fn flight_mode(&self) -> ShipFlightMode {
        let ship = self.ship.lock().unwrap();
        ship.nav.flight_mode.clone()
    }
    pub fn nav_status(&self) -> ShipNavStatus {
        let ship = self.ship.lock().unwrap();
        ship.nav.status.clone()
    }
    pub fn engine_speed(&self) -> i64 {
        let ship = self.ship.lock().unwrap();
        ship.engine.speed
    }
    pub fn fuel_capacity(&self) -> i64 {
        let ship = self.ship.lock().unwrap();
        ship.fuel.capacity
    }
    pub fn current_fuel(&self) -> i64 {
        let ship = self.ship.lock().unwrap();
        ship.fuel.current
    }
    pub fn cargo_capacity(&self) -> i64 {
        let ship = self.ship.lock().unwrap();
        ship.cargo.capacity
    }
    pub fn cargo_units(&self) -> i64 {
        let ship = self.ship.lock().unwrap();
        ship.cargo.units
    }
    pub fn waypoint(&self) -> WaypointSymbol {
        let ship = self.ship.lock().unwrap();
        ship.nav.waypoint_symbol.clone()
    }
    pub fn system(&self) -> SystemSymbol {
        let ship = self.ship.lock().unwrap();
        ship.nav.system_symbol.clone()
    }
    pub fn cargo_empty(&self) -> bool {
        let ship = self.ship.lock().unwrap();
        ship.cargo.units == 0
    }
    pub async fn emit_ship(&self) {
        let ship = self.ship();
        self.agent_controller
            .emit_event(&Event::ShipUpdate(ship))
            .await;
    }
    pub async fn set_orbit_status(&self) {
        {
            let mut ship = self.ship.lock().unwrap();
            ship.nav.status = InOrbit;
        }
        self.emit_ship().await;
    }
    pub async fn update_nav(&self, nav: ShipNav) {
        {
            let mut ship = self.ship.lock().unwrap();
            ship.nav = nav;
        }
        self.emit_ship().await;
    }
    pub async fn update_fuel(&self, fuel: ShipFuel) {
        {
            let mut ship = self.ship.lock().unwrap();
            ship.fuel = fuel;
        }
        self.emit_ship().await;
    }
    pub async fn update_cargo(&self, cargo: ShipCargo) {
        {
            let mut ship = self.ship.lock().unwrap();
            ship.cargo = cargo;
        }
        self.emit_ship().await;
    }
    pub async fn update_cooldown(&self, cooldown: ShipCooldown) {
        {
            let mut ship = self.ship.lock().unwrap();
            ship.cooldown = cooldown;
        }
        self.emit_ship().await;
    }
    pub fn cargo_first_item(&self) -> Option<ShipCargoItem> {
        let ship = self.ship.lock().unwrap();
        ship.cargo.inventory.first().cloned()
    }
    pub fn cargo_good_count(&self, good: &str) -> i64 {
        let ship = self.ship.lock().unwrap();
        ship.cargo
            .inventory
            .iter()
            .find(|g| g.symbol == *good)
            .map(|g| g.units)
            .unwrap_or(0)
    }
    pub fn cargo_space_available(&self) -> i64 {
        let ship = self.ship.lock().unwrap();
        ship.cargo.capacity - ship.cargo.units
    }
    pub fn cargo_map(&self) -> std::collections::BTreeMap<String, i64> {
        let ship = self.ship.lock().unwrap();
        ship.cargo
            .inventory
            .iter()
            .map(|g| (g.symbol.clone(), g.units))
            .collect()
    }

    pub fn debug(&self, msg: &str) {
        debug!("[{}] {}", self.ship_symbol, msg);
    }

    pub async fn orbit(&self) {
        if self.nav_status() == InOrbit {
            return;
        }
        let uri = format!("/my/ships/{}/orbit", self.ship_symbol);
        let resp: Data<OrbitResponse> = self.api_client.post(&uri, &json!({})).await;
        self.update_nav(resp.data.nav).await;
    }

    pub async fn dock(&self) {
        if self.nav_status() == Docked {
            return;
        }
        let uri = format!("/my/ships/{}/dock", self.ship_symbol);
        let resp: Data<OrbitResponse> = self.api_client.post(&uri, &json!({})).await;
        self.update_nav(resp.data.nav).await;
    }

    pub async fn set_flight_mode(&self, mode: ShipFlightMode) {
        #[derive(Debug, Clone, Serialize, Deserialize)]
        struct NavUpdateResponse {
            nav: ShipNav,
            fuel: ShipFuel,
            events: Vec<ShipConditionEvent>,
        }

        if self.flight_mode() == mode {
            return;
        }
        self.debug(&format!("Setting flight mode to {:?}", mode));
        let uri = format!("/my/ships/{}/nav", self.ship_symbol);
        let response: Data<NavUpdateResponse> = self
            .api_client
            .patch(&uri, &json!({ "flightMode": mode }))
            .await;
        let nav = response.data.nav;
        let fuel = response.data.fuel;
        let events = response.data.events;
        self.update_nav(nav).await;
        self.update_fuel(fuel).await;
        self.handle_ship_condition_events(&events);
    }

    pub fn is_in_transit(&self) -> bool {
        let arrival_time = self.ship.lock().unwrap().nav.route.arrival;
        let now = chrono::Utc::now();
        arrival_time >= now
    }

    pub fn set_nav_status(&self, status: ShipNavStatus) {
        let mut ship = self.ship.lock().unwrap();
        ship.nav.status = status;
    }

    pub async fn wait_for_transit(&self) {
        let arrival_time = { self.ship.lock().unwrap().nav.route.arrival };
        let now = chrono::Utc::now();
        let wait_time = arrival_time - now + chrono::Duration::try_seconds(1).unwrap();
        if wait_time > chrono::Duration::try_seconds(0).unwrap() {
            self.debug(&format!(
                "Waiting for transit: {} seconds",
                wait_time.num_seconds()
            ));
            tokio::time::sleep(wait_time.to_std().unwrap()).await;
        }
    }
    pub async fn wait_for_cooldown(&self) {
        let cooldown = { self.ship.lock().unwrap().cooldown.clone() };
        if let Some(expiration) = cooldown.expiration {
            let now = chrono::Utc::now();
            let wait_time = expiration - now + chrono::Duration::try_seconds(1).unwrap();
            if wait_time > chrono::Duration::try_seconds(0).unwrap() {
                self.debug(&format!(
                    "Waiting for cooldown: {} seconds",
                    wait_time.num_seconds()
                ));
                tokio::time::sleep(wait_time.to_std().unwrap()).await;
            }
        }
    }

    async fn trade_good(&self, _type: &str, good: &str, units: i64, adjust_reserved_credits: bool) {
        assert!(!self.is_in_transit(), "Ship is in transit");
        match _type {
            "purchase" => {
                assert!(
                    units <= self.cargo_capacity(),
                    "Ship can't hold that much cargo"
                );
                self.debug(&format!("Buying {} units of {}", units, good));
            }
            "sell" => {
                self.debug(&format!("Selling {} units of {}", units, good));
            }
            _ => {
                panic!("Invalid trade type: {}", _type);
            }
        }
        self.dock().await;
        let uri = format!("/my/ships/{}/{}", self.ship_symbol, _type);
        let body = json!({
            "symbol": good,
            "units": units,
        });
        let TradeResponse { cargo, agent, transaction } = self
            .api_client
            .post::<Data<TradeResponse>, _>(&uri, &body)
            .await
            .data;
        self.update_cargo(cargo).await;
        self.agent_controller.update_agent(agent).await;
        if adjust_reserved_credits {
            let units = if _type == "purchase" { units } else { -units };
            self.agent_controller.ledger.register_goods_change(
                &self.ship_symbol,
                &transaction.trade_symbol,
                units,
                transaction.price_per_unit,
            );
        }
        self.debug(&format!(
            "{} {} {} for ${} (total ${})",
            transaction._type,
            transaction.units,
            transaction.trade_symbol,
            transaction.price_per_unit,
            transaction.total_price
        ));
    }

    pub async fn buy_goods(&self, good: &str, units: i64, adjust_reserved_credits: bool) {
        self.trade_good("purchase", good, units, adjust_reserved_credits).await;
    }

    pub async fn sell_goods(&self, good: &str, units: i64, adjust_reserved_credits: bool) {
        self.trade_good("sell", good, units, adjust_reserved_credits).await;
    }

    pub async fn sell_all_cargo(&self) {
        self.refresh_market().await;
        let market = self.universe.get_market(&self.waypoint()).await.unwrap();
        while let Some(cargo_item) = self.cargo_first_item() {
            let market_good = market
                .data
                .trade_goods
                .iter()
                .find(|g| g.symbol == cargo_item.symbol)
                .unwrap();
            let units = min(market_good.trade_volume, cargo_item.units);
            assert!(units > 0);
            self.sell_goods(&cargo_item.symbol, units, false).await;
            let new_units = self.cargo_good_count(&cargo_item.symbol);
            assert!(new_units == cargo_item.units - units);
        }
        self.refresh_market().await;
    }

    pub async fn jettison_cargo(&self, good: &str, units: i64) {
        assert!(!self.is_in_transit(), "Ship is in transit");
        self.debug(format!("Jettisoning {} {}", units, good).as_str());
        let uri = format!("/my/ships/{}/jettison", self.ship_symbol);
        let body = json!({
            "symbol": good,
            "units": units,
        });
        let cargo = self.api_client.post::<Data<ShipCargo>, _>(&uri, &body).await.data;
        self.update_cargo(cargo).await;
    }

    // Fuel is bought in multiples of 100, so refuel as the highest multiple of 100
    // or to full if that wouldn't reach the required_fuel amount
    //
    // If from_cargo is true, refuel from cargo, and we must check after the refuel whether the refuel suceeded
    // Whereas if buying from market, we can safely assume we can obtain the required amount
    pub async fn refuel(&self, required_fuel: i64, from_cargo: bool) {
        #[derive(Debug, Clone, Serialize, Deserialize)]
        struct RefuelResponse {
            agent: Agent,
            fuel: ShipFuel,
            transaction: MarketTransaction,
            cargo: ShipCargo,
        }

        assert!(!self.is_in_transit(), "Ship is in transit");
        assert!(
            required_fuel <= self.fuel_capacity(),
            "Ship can't hold that much fuel"
        );
        if self.current_fuel() >= required_fuel {
            return;
        }

        let current = self.current_fuel();
        let capacity = self.fuel_capacity();
        let max_refuel_units = match from_cargo {
            true => 100 * self.cargo_good_count("FUEL"),
            false => i64::MAX,
        };
        if max_refuel_units == 0 {
            self.debug("No fuel in cargo to refuel");
            return;
        }
        let mut units = {
            let missing_fuel = capacity - current;
            // round down to the nearest 100, so we don't buy more than we need
            let units = (missing_fuel / 100) * 100;
            if units + current < required_fuel {
                missing_fuel
            } else {
                units
            }
        };
        units = min(units, max_refuel_units);
        self.dock().await;
        self.debug(&format!(
            "Refueling {} to {}/{}",
            units,
            current + units,
            capacity
        ));
        let uri = format!("/my/ships/{}/refuel", self.ship_symbol);
        let body = json!({
            "units": units,
            "fromCargo": from_cargo,
        });

        let cargo_fuel = self.cargo_good_count("FUEL");
        let expected_cargo_fuel = if from_cargo {
            let cargo_units = (units + 99) / 100;
            assert!(cargo_fuel >= cargo_units);
            cargo_fuel - cargo_units
        } else {
            cargo_fuel
        };
        let RefuelResponse { fuel, agent, cargo, transaction: _ } = self
            .api_client
            .post::<Data<RefuelResponse>, _>(&uri, &body)
            .await
            .data;
        self.update_fuel(fuel).await;
        self.update_cargo(cargo).await;
        self.agent_controller.update_agent(agent).await;
        assert_eq!(self.cargo_good_count("FUEL"), expected_cargo_fuel);
    }

    pub async fn full_load_cargo(&self, good: &str) {
        let cargo_units = self.cargo_good_count(good);
        assert_eq!(cargo_units, self.cargo_units());

        let buy_units = self.cargo_capacity() - cargo_units;
        if buy_units > 0 {
            // Makes assumptions about the TV of the good
            self.buy_goods(good, buy_units, false).await;
            self.refresh_market().await;
        }
    }

    async fn navigate(&self, flight_mode: ShipFlightMode, waypoint: &WaypointSymbol) {
        assert!(!self.is_in_transit(), "Ship is already in transit");
        if self.waypoint() == *waypoint {
            return;
        }
        assert_eq!(self.waypoint().system(), waypoint.system());
        self.set_flight_mode(flight_mode).await;
        self.orbit().await;
        self.debug(&format!("Navigating to waypoint: {}", waypoint));
        let uri = format!("/my/ships/{}/navigate", self.ship_symbol);
        let NavigateResponse { nav, fuel, events } = self
            .api_client
            .post::<Data<NavigateResponse>, _>(&uri, &json!({ "waypointSymbol": waypoint }))
            .await
            .data;
        self.handle_ship_condition_events(&events);
        self.update_nav(nav).await;
        self.update_fuel(fuel).await;
        self.wait_for_transit().await;
        self.set_orbit_status().await;
    }

    pub async fn warp(&self, flight_mode: ShipFlightMode, waypoint: &WaypointSymbol) {
        assert!(!self.is_in_transit(), "Ship is already in transit");
        if self.waypoint() == *waypoint {
            return;
        }
        assert_ne!(self.waypoint().system(), waypoint.system());
        self.set_flight_mode(flight_mode).await;
        self.orbit().await;
        self.debug(&format!("Warp to waypoint: {}", waypoint));
        let uri = format!("/my/ships/{}/warp", self.ship_symbol);
        let NavigateResponse { nav, fuel, events } = self
            .api_client
            .post::<Data<NavigateResponse>, _>(&uri, &json!({ "waypointSymbol": waypoint }))
            .await
            .data;
        self.handle_ship_condition_events(&events);
        self.update_nav(nav).await;
        self.update_fuel(fuel).await;
        self.wait_for_transit().await;
        self.set_orbit_status().await;
    }

    pub async fn jump(&self, waypoint: &WaypointSymbol) {
        #[derive(Debug, Clone, Serialize, Deserialize)]
        struct JumpResponse {
            nav: ShipNav,
            cooldown: ShipCooldown,
            transaction: MarketTransaction,
            agent: Agent,
        }

        assert!(!self.is_in_transit(), "Ship is in transit");
        self.wait_for_cooldown().await;
        self.orbit().await;
        self.debug(&format!("Jumping to waypoint: {}", waypoint));
        let uri = format!("/my/ships/{}/jump", self.ship_symbol);
        let body = json!({ "waypointSymbol": waypoint });
        let JumpResponse { nav, cooldown, agent, transaction: _ } = self
            .api_client
            .post::<Data<JumpResponse>, _>(&uri, &body)
            .await
            .data;
        self.update_nav(nav).await;
        self.agent_controller.update_agent(agent).await;
        self.update_cooldown(cooldown).await;
    }

    // Navigation between two waypoints
    pub async fn goto_waypoint(&self, target: &WaypointSymbol) {
        assert!(!self.is_in_transit(), "Ship is already in transit");
        if self.fuel_capacity() == 0 {
            self.navigate(ShipFlightMode::Cruise, target).await;
            self.debug(&format!("Arrived at waypoint: {}", target));
            return;
        }
        if self.waypoint() == *target {
            return;
        }
        let route = self
            .universe
            .get_route(
                &self.waypoint(),
                target,
                self.engine_speed(),
                self.current_fuel(),
                self.fuel_capacity(),
            )
            .await;
        for (waypoint, edge, a_market, b_market) in route.hops {
            // calculate fuel required before leaving
            let required_fuel = if b_market {
                edge.fuel_cost
            } else {
                assert!(waypoint == *target);
                edge.fuel_cost + route.req_terminal_fuel
            };
            if self.current_fuel() < required_fuel {
                assert!(a_market);
                self.refuel(required_fuel, false).await;
            }
            self.navigate(edge.flight_mode, &waypoint).await;
            self.debug(&format!("Arrived at waypoint: {}", waypoint));
        }
    }

    pub async fn supply_construction(&self, good: &str, units: i64) {
        #[derive(Debug, Clone, Serialize, Deserialize)]
        struct SupplyConstructionResponse {
            cargo: ShipCargo,
            construction: Construction,
        }

        assert!(!self.is_in_transit(), "Ship is in transit");
        self.dock().await;
        self.debug(&format!("Constructing {} units of {}", units, good));
        let uri = format!(
            "/systems/{}/waypoints/{}/construction/supply",
            self.system(),
            self.waypoint()
        );
        let body = json!({
            "shipSymbol": self.ship_symbol,
            "tradeSymbol": good,
            "units": units,
        });
        let SupplyConstructionResponse { cargo, construction } = self
            .api_client
            .post::<Data<SupplyConstructionResponse>, _>(&uri, &body)
            .await
            .data;
        self.update_cargo(cargo).await;
        self.universe.update_construction(&construction).await;
    }

    pub async fn refresh_market(&self) {
        assert!(!self.is_in_transit());
        let waypoint = self.waypoint();
        let system = self.system();
        self.debug(&format!("Refreshing market at waypoint {}", &waypoint));
        let uri = format!("/systems/{}/waypoints/{}/market", &system, &waypoint);
        let response: Data<Market> = self.api_client.get(&uri).await;
        let market = WithTimestamp::<Market> {
            timestamp: chrono::Utc::now(),
            data: response.data,
        };
        self.universe.save_market(&waypoint, market).await;
    }

    pub async fn refresh_shipyard(&self) {
        assert!(!self.is_in_transit());
        let waypoint = self.waypoint();
        let system = self.system();
        self.debug(&format!("Refreshing shipyard at waypoint {}", &waypoint));
        let uri = format!("/systems/{}/waypoints/{}/shipyard", &system, &waypoint);
        let response: Data<Shipyard> = self.api_client.get(&uri).await;
        let shipyard = WithTimestamp::<Shipyard> {
            timestamp: chrono::Utc::now(),
            data: response.data,
        };
        self.universe.save_shipyard(&waypoint, shipyard).await;
    }

    pub async fn survey(&self) {
        #[derive(Debug, Clone, Serialize, Deserialize)]
        struct SurveyResponse {
            cooldown: ShipCooldown,
            surveys: Vec<Survey>,
        }

        assert!(!self.is_in_transit());
        self.wait_for_cooldown().await;
        self.debug(&format!("Surveying {}", self.waypoint()));
        let uri = format!("/my/ships/{}/survey", self.ship_symbol);
        let SurveyResponse { cooldown, surveys } = self
            .api_client
            .post::<Data<SurveyResponse>, _>(&uri, &json!({}))
            .await
            .data;
        for survey in &surveys {
            let deposits = survey
                .deposits
                .iter()
                .map(|d| d.symbol.clone())
                .collect::<Vec<_>>()
                .join(", ");
            self.debug(&format!("Surveyed {} {}", survey.size, deposits));
        }
        self.update_cooldown(cooldown).await;
        self.agent_controller
            .survey_manager
            .insert_surveys(surveys)
            .await;
    }

    pub async fn execute_action(&self, action: &Action) {
        match action {
            Action::RefreshMarket => self.refresh_market().await,
            Action::RefreshShipyard => self.refresh_shipyard().await,
            // Interpret this action as units is the target
            Action::BuyGoods(good, units) => {
                let good_count = self.cargo_good_count(good);
                let mut remaining_to_buy = units - good_count;
                self.refresh_market().await;
                while remaining_to_buy > 0 {
                    let market = self.universe.get_market(&self.waypoint()).await.unwrap();
                    let trade = market
                        .data
                        .trade_goods
                        .iter()
                        .find(|g| g.symbol == *good)
                        .unwrap();
                    let buy_units = min(trade.trade_volume, remaining_to_buy);
                    self.buy_goods(good, buy_units, true).await;
                    self.refresh_market().await;
                    remaining_to_buy -= buy_units;
                }
            }
            // Always sell to 0
            Action::SellGoods(good, _units) => {
                // We need to handle falling trade volume
                let good_count = self.cargo_good_count(good);
                let mut remaining_to_sell = good_count; // min(*units, good_count);
                self.refresh_market().await;
                while remaining_to_sell > 0 {
                    let market = self.universe.get_market(&self.waypoint()).await.unwrap();
                    let trade = market
                        .data
                        .trade_goods
                        .iter()
                        .find(|g| g.symbol == *good)
                        .unwrap();
                    let sell_units = min(trade.trade_volume, remaining_to_sell);
                    self.sell_goods(good, sell_units, true).await;
                    self.refresh_market().await;
                    remaining_to_sell -= sell_units;
                }
            }
            Action::TryBuyShips => {
                assert!(!self.is_in_transit());
                info!("Starting buy task for ship {}", self.ship_symbol);
                self.dock().await; // don't need to dock, but do so anyway to clear 'InTransit' status
                let (bought, _shipyard_waypoints) = self
                    .agent_controller
                    .try_buy_ships(Some(self.ship_symbol.clone()))
                    .await;
                info!("Buy task resulted in {} ships bought", bought.len());
                for ship_symbol in bought {
                    self.debug(&format!("{} Bought ship {}", self.ship_symbol, ship_symbol));
                    self.agent_controller._spawn_run_ship(ship_symbol).await;
                }
            }
            Action::DeliverConstruction(good, units) => {
                // todo, handle case where construction materials no longer needed
                self.supply_construction(good, *units).await;
            }
            _ => {
                panic!("Action not implemented: {:?}", action);
            }
        }
    }

    pub async fn transfer_cargo(&self) {
        assert!(!self.is_in_transit(), "Ship is in transit");
        self.orbit().await;
        let cargo = {
            let ship = self.ship.lock().unwrap();
            ship.cargo
                .inventory
                .iter()
                .map(|g| (g.symbol.clone(), g.units))
                .collect()
        };
        self.agent_controller
            .cargo_broker
            .transfer_cargo(&self.ship_symbol, &self.waypoint(), cargo)
            .await;
    }

    pub async fn receive_cargo(&self) {
        self.orbit().await;
        assert!(!self.is_in_transit(), "Ship is in transit");
        let space = self.cargo_space_available();
        self.agent_controller
            .cargo_broker
            .receive_cargo(&self.ship_symbol, &self.waypoint(), space)
            .await;
    }

    pub async fn siphon(&self) {
        #[derive(Debug, Clone, Serialize, Deserialize)]
        struct SiphonResponse {
            cargo: ShipCargo,
            cooldown: ShipCooldown,
            siphon: Value,
            events: Vec<ShipConditionEvent>,
        }
        assert!(!self.is_in_transit(), "Ship is in transit");
        self.orbit().await;
        self.wait_for_cooldown().await;
        self.debug("Siphoning");
        let uri = format!("/my/ships/{}/siphon", self.ship_symbol);
        let body = json!({});
        let SiphonResponse { cargo, cooldown, siphon, events } = self
            .api_client
            .post::<Data<SiphonResponse>, _>(&uri, &body)
            .await
            .data;
        let good = siphon["yield"]["symbol"].as_str().unwrap();
        let units = siphon["yield"]["units"].as_i64().unwrap();
        self.handle_ship_condition_events(&events);
        self.debug(&format!("Siphoned {} units of {}", units, good));
        self.update_cooldown(cooldown).await;
        self.update_cargo(cargo).await;
    }

    pub async fn extract_survey(&self, survey: &KeyedSurvey) {
        assert!(!self.is_in_transit(), "Ship is in transit");
        // self.orbit().await;
        self.wait_for_cooldown().await;
        self.debug(&format!("Extracting survey {}", survey.uuid));
        let uri = format!("/my/ships/{}/extract/survey", self.ship_symbol);
        let req_body = &survey.survey;
        // let mut response: Value = self.api_client.post(&uri, body).await;

        let (code, resp_body): (StatusCode, Result<Value, String>) = self
            .api_client
            .request(Method::POST, &uri, Some(req_body))
            .await;
        match code {
            StatusCode::CREATED => {
                let mut response = resp_body.unwrap();
                let cargo: ShipCargo =
                    serde_json::from_value(response["data"]["cargo"].take()).unwrap();
                let cooldown: ShipCooldown =
                    serde_json::from_value(response["data"]["cooldown"].take()).unwrap();
                let extraction: Value =
                    serde_json::from_value(response["data"]["extraction"].take()).unwrap();
                let events = serde_json::from_value(response["data"]["events"].take()).unwrap();
                self.handle_ship_condition_events(&events);
                let good = extraction["yield"]["symbol"].as_str().unwrap();
                let units = extraction["yield"]["units"].as_i64().unwrap();
                self.debug(&format!("Extracted {} units of {}", units, good));
                self.update_cooldown(cooldown).await;
                self.update_cargo(cargo).await;
            }
            StatusCode::BAD_REQUEST | StatusCode::CONFLICT => {
                let response: Value = serde_json::from_str(&resp_body.unwrap_err()).unwrap();
                // variety of responses we might get here: exhausted, expired, asteroid overmined
                let code = response["error"]["code"].as_i64().unwrap();
                if code == 4221 {
                    // Request failed: 400 {"error":{"message":"Ship survey failed. Target signature is no longer in range or valid.","code":4221}}
                    self.debug(
                        "Extraction failed: Target signature is no longer in range or valid",
                    );
                    self.agent_controller
                        .survey_manager
                        .remove_survey(&survey)
                        .await;
                } else if code == 4224 {
                    // Request failed: 409 Err("{\"error\":{\"message\":\"Ship extract failed. Survey X1-FM95-CD5Z-BEC3E1 has been exhausted.\",\"code\":4224}}")
                    self.debug("Extraction failed: Survey has been exhausted");
                    self.agent_controller
                        .survey_manager
                        .remove_survey(&survey)
                        .await;
                } else {
                    panic!(
                        "Request failed: {} {} {}\nbody: {:?}",
                        code,
                        Method::POST,
                        uri,
                        response
                    );
                }
            }
            _ => panic!(
                "Request failed: {} {} {}\nbody: {:?}",
                code.as_u16(),
                Method::POST,
                uri,
                resp_body
            ),
        };
    }

    pub async fn scrap(&self) {
        #[derive(Debug, Clone, Serialize, Deserialize)]
        struct ScrapResponse {
            agent: Agent,
            transaction: ScrapTransaction,
        }

        assert!(!self.is_in_transit(), "Ship is in transit");
        self.dock().await;
        self.debug("Scrapping Ship");
        let uri = format!("/my/ships/{}/scrap", self.ship_symbol);
        let ScrapResponse { agent, transaction } = self
            .api_client
            .post::<Data<ScrapResponse>, _>(&uri, &json!({}))
            .await
            .data;
        info!(
            "{} Scrapped ship for ${}",
            self.ship_symbol, transaction.total_price
        );
        self.agent_controller.update_agent(agent).await;
    }

    pub fn handle_ship_condition_events(&self, events: &Vec<ShipConditionEvent>) {
        for e in events {
            self.debug(&format!("ENCOUNTERED SHIP EVENT: {:?}", e));
        }
    }

    pub fn set_state_description(&self, desc: &str) {
        self.agent_controller
            .set_state_description(&self.ship_symbol, desc)
    }
}
