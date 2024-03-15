use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};

use crate::api_client::ApiClient;
use crate::data::DataClient;
use crate::models::{
    Construction, Faction, Market, MarketRemoteView, Shipyard, ShipyardRemoteView, System,
    SystemSymbol, Waypoint, WaypointSymbol, WithTimestamp,
};
use crate::pathfinding::{Pathfinding, Route};
use std::collections::BTreeMap;
use std::sync::Arc;

pub enum WaypointFilter {
    Imports(String),
    Exports(String),
    Exchanges(String),
    // waypoint traits
    Market,
    Shipyard,
    // waypoint types
    GasGiant,
    EngineeredAsteroid,
    JumpGate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JumpGateInfo {
    pub timestamp: DateTime<Utc>,
    pub connections: JumpGateConnections,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum JumpGateConnections {
    Charted(Vec<WaypointSymbol>),
    Uncharted,
}

#[derive(Clone)]
pub struct Universe {
    api_client: ApiClient,
    db: DataClient,

    constructions: Arc<DashMap<WaypointSymbol, Arc<WithTimestamp<Option<Construction>>>>>,
    remote_markets: Arc<DashMap<WaypointSymbol, MarketRemoteView>>,
    markets: Arc<DashMap<WaypointSymbol, Option<Arc<WithTimestamp<Market>>>>>,
    remote_shipyards: Arc<DashMap<WaypointSymbol, ShipyardRemoteView>>,
    shipyards: Arc<DashMap<WaypointSymbol, Option<Arc<WithTimestamp<Shipyard>>>>>,
    factions: DashMap<String, Faction>,
    jumpgates: Arc<DashMap<WaypointSymbol, JumpGateInfo>>,
}

impl Universe {
    pub fn new(api_client: &ApiClient, db: &DataClient) -> Self {
        Self {
            api_client: api_client.clone(),
            db: db.clone(),
            constructions: Arc::new(DashMap::new()),
            remote_markets: Arc::new(DashMap::new()),
            markets: Arc::new(DashMap::new()),
            remote_shipyards: Arc::new(DashMap::new()),
            shipyards: Arc::new(DashMap::new()),
            factions: DashMap::new(),
            jumpgates: Arc::new(DashMap::new()),
        }
    }

    pub async fn get_market(
        &self,
        waypoint_symbol: &WaypointSymbol,
    ) -> Option<Arc<WithTimestamp<Market>>> {
        match self.markets.get(waypoint_symbol) {
            Some(market) => market.clone(),
            None => {
                let market = self
                    .db
                    .get_market(waypoint_symbol)
                    .await
                    .map(|market| Arc::new(market));
                self.markets.insert(waypoint_symbol.clone(), market.clone());
                market
            }
        }
    }

    pub async fn save_market(
        &self,
        waypoint_symbol: &WaypointSymbol,
        market: WithTimestamp<Market>,
    ) {
        self.markets
            .insert(waypoint_symbol.clone(), Some(Arc::new(market.clone())));
        self.db.save_market(waypoint_symbol, &market).await;
        self.db.insert_market_trades(&market).await;
        self.db.upsert_market_transactions(&market).await;
    }

    pub async fn get_shipyard(
        &self,
        waypoint_symbol: &WaypointSymbol,
    ) -> Option<Arc<WithTimestamp<Shipyard>>> {
        match self.shipyards.get(waypoint_symbol) {
            Some(shipyard) => shipyard.clone(),
            None => {
                let shipyard = self
                    .db
                    .get_shipyard(waypoint_symbol)
                    .await
                    .map(|x| Arc::new(x));
                self.shipyards
                    .insert(waypoint_symbol.clone(), shipyard.clone());
                shipyard
            }
        }
    }

    pub async fn save_shipyard(
        &self,
        waypoint_symbol: &WaypointSymbol,
        shipyard: WithTimestamp<Shipyard>,
    ) {
        self.shipyards
            .insert(waypoint_symbol.clone(), Some(Arc::new(shipyard.clone())));
        self.db.save_shipyard(waypoint_symbol, &shipyard).await;
    }

    // load Optional<Construction> from db, or fetch from api
    // we should only do initial fetch from api once, and rely on other processes to update
    pub async fn load_construction(
        &self,
        symbol: &WaypointSymbol,
    ) -> WithTimestamp<Option<Construction>> {
        match self.db.get_construction(symbol).await {
            Some(site) => site,
            None => {
                let site = self.api_client.get_construction(symbol).await;
                self.db.save_construction(symbol, &site).await;
                site
            }
        }
    }

    pub async fn get_construction(
        &self,
        symbol: &WaypointSymbol,
    ) -> Arc<WithTimestamp<Option<Construction>>> {
        match self.constructions.get(symbol) {
            Some(construction) => construction.clone(),
            None => {
                let construction = self.load_construction(symbol).await;
                let construction = Arc::new(construction);
                self.constructions
                    .insert(symbol.clone(), construction.clone());
                construction
            }
        }
    }

    pub async fn update_construction(&self, construction: &Construction) {
        let symbol = &construction.symbol;
        let construction = WithTimestamp {
            data: Some(construction.clone()),
            timestamp: chrono::Utc::now(),
        };
        self.constructions
            .insert(symbol.clone(), Arc::new(construction.clone()));
        self.db.save_construction(symbol, &construction).await;
    }

    pub async fn get_system(&self, symbol: &SystemSymbol) -> System {
        // cache behaviour: this data will never go stale
        // check data cache first then use api
        match self.db.get_system(symbol).await {
            Some(system) => system,
            None => {
                let system = self.api_client.get_system(symbol).await;
                self.db.save_system(&system.symbol, &system).await;
                system
            }
        }
    }

    // !! needs caching layer
    pub async fn get_system_waypoints(&self, symbol: &SystemSymbol) -> Vec<Waypoint> {
        // cache behaviour: waypoint data mostly not go stale, except for fields: 'modifiers' and 'is_under_construction'
        // also: chart and traits if UNCHARTED
        // check data cache first then use api
        match self.db.get_system_waypoints(symbol).await {
            Some(waypoints) => waypoints,
            None => {
                let waypoints = self.api_client.get_system_waypoints(symbol).await;
                self.db.save_system_waypoints(symbol, &waypoints).await;
                waypoints
            }
        }
    }

    pub async fn get_system_markets(
        &self,
        symbol: &SystemSymbol,
    ) -> Vec<(MarketRemoteView, Option<Arc<WithTimestamp<Market>>>)> {
        let waypoints = self.get_system_waypoints(symbol).await;
        let mut markets = Vec::new();
        for waypoint in &waypoints {
            if waypoint.is_market() {
                let market_remote = self.get_market_remote(&waypoint.symbol).await;
                let market_opt = self.get_market(&waypoint.symbol).await;
                markets.push((market_remote, market_opt));
            }
        }
        markets
    }

    pub async fn get_system_shipyards(
        &self,
        symbol: &SystemSymbol,
    ) -> Vec<(ShipyardRemoteView, Option<Arc<WithTimestamp<Shipyard>>>)> {
        let waypoints = self.get_system_waypoints(symbol).await;
        let mut shipyards = Vec::new();
        for waypoint in &waypoints {
            if waypoint.is_shipyard() {
                let shipyard_remote = self.get_shipyard_remote(&waypoint.symbol).await;
                let shipyard_opt = self.get_shipyard(&waypoint.symbol).await;
                shipyards.push((shipyard_remote, shipyard_opt));
            }
        }
        shipyards
    }

    pub async fn get_system_markets_remote(&self, symbol: &SystemSymbol) -> Vec<MarketRemoteView> {
        let waypoints = self.get_system_waypoints(symbol).await;
        let mut markets = Vec::new();
        for waypoint in &waypoints {
            if waypoint.is_market() {
                let market_remote = self.get_market_remote(&waypoint.symbol).await;
                markets.push(market_remote);
            }
        }
        markets
    }

    pub async fn get_system_shipyards_remote(
        &self,
        symbol: &SystemSymbol,
    ) -> Vec<ShipyardRemoteView> {
        let waypoints = self.get_system_waypoints(symbol).await;
        let mut shipyards = Vec::new();
        for waypoint in &waypoints {
            if waypoint.is_shipyard() {
                let shipyard_remote = self.get_shipyard_remote(&waypoint.symbol).await;
                shipyards.push(shipyard_remote);
            }
        }
        shipyards
    }

    // get waypoints, but don't use api
    pub async fn get_system_waypoints_no_fetch(
        &self,
        symbol: &SystemSymbol,
    ) -> Option<Vec<Waypoint>> {
        // needs caching layer
        self.db.get_system_waypoints(symbol).await
    }

    pub async fn get_waypoint(&self, symbol: &WaypointSymbol) -> Waypoint {
        let system_waypoints = self.get_system_waypoints(&symbol.system()).await;
        system_waypoints
            .into_iter()
            .find(|waypoint| &waypoint.symbol == symbol)
            .unwrap()
    }

    pub async fn all_systems(&self) -> Vec<System> {
        let db_key = "systems.json";
        match self.db.get_value(db_key).await {
            Some(systems) => systems,
            None => {
                let systems: Vec<System> = self.api_client.get("systems.json").await;
                self.db.set_value(db_key, &systems).await;
                systems
            }
        }
    }

    pub async fn get_market_remote(&self, symbol: &WaypointSymbol) -> MarketRemoteView {
        // Layer 1 - check cache
        if let Some(market) = &self.remote_markets.get(symbol) {
            return market.value().clone();
        }
        // Layer 2 - check db
        if let Some(market) = self.db.get_market_remote(symbol).await {
            self.remote_markets.insert(symbol.clone(), market.clone());
            return market;
        }
        // Layer 3 - fetch from api
        let market = self.api_client.get_market_remote(symbol).await;
        self.db.save_market_remote(symbol, &market).await;
        self.remote_markets.insert(symbol.clone(), market.clone());
        market
    }

    pub async fn get_shipyard_remote(&self, symbol: &WaypointSymbol) -> ShipyardRemoteView {
        // Layer 1 - check cache
        if let Some(shipyard) = &self.remote_shipyards.get(symbol) {
            return shipyard.value().clone();
        }
        // Layer 2 - check db
        if let Some(shipyard) = self.db.get_shipyard_remote(symbol).await {
            self.remote_shipyards
                .insert(symbol.clone(), shipyard.clone());
            return shipyard;
        }
        // Layer 3 - fetch from api
        let shipyard = self.api_client.get_shipyard_remote(symbol).await;
        self.db.save_shipyard_remote(symbol, &shipyard).await;
        self.remote_shipyards
            .insert(symbol.clone(), shipyard.clone());
        shipyard
    }

    pub async fn search_shipyards(
        &self,
        system_symbol: &SystemSymbol,
        ship_model: &str,
    ) -> Vec<(WaypointSymbol, i64)> {
        let waypoints = self.get_system_waypoints(system_symbol).await;
        let mut shipyards = Vec::new();
        for waypoint in waypoints {
            if !waypoint.is_shipyard() {
                continue;
            }
            if let Some(shipyard) = self.get_shipyard(&waypoint.symbol).await {
                if let Some(ship) = shipyard
                    .data
                    .ships
                    .iter()
                    .find(|ship| ship.ship_type == ship_model)
                {
                    shipyards.push((waypoint.symbol.clone(), ship.purchase_price));
                }
            }
        }
        shipyards
    }

    async fn matches_filter(&self, waypoint: &Waypoint, filter: &WaypointFilter) -> bool {
        match filter {
            WaypointFilter::Imports(good) => {
                if !waypoint.is_market() {
                    return false;
                }
                let market = self.get_market_remote(&waypoint.symbol).await;
                market.imports.iter().any(|import| import.symbol == *good)
            }
            WaypointFilter::Exports(good) => {
                if !waypoint.is_market() {
                    return false;
                }
                let market = self.get_market_remote(&waypoint.symbol).await;
                market.exports.iter().any(|export| export.symbol == *good)
            }
            WaypointFilter::Exchanges(good) => {
                if !waypoint.is_market() {
                    return false;
                }
                let market = self.get_market_remote(&waypoint.symbol).await;
                market
                    .exchange
                    .iter()
                    .any(|exchange| exchange.symbol == *good)
            }
            WaypointFilter::Market => waypoint.is_market(),
            WaypointFilter::Shipyard => waypoint.is_shipyard(),
            WaypointFilter::GasGiant => waypoint.is_gas_giant(),
            WaypointFilter::EngineeredAsteroid => waypoint.is_engineered_asteroid(),
            WaypointFilter::JumpGate => waypoint.is_jump_gate(),
        }
    }

    pub async fn search_waypoints(
        &self,
        system_symbol: &SystemSymbol,
        filters: &[WaypointFilter],
    ) -> Vec<Waypoint> {
        let waypoints = self.get_system_waypoints(system_symbol).await;
        let mut filtered = Vec::new();
        for waypoint in waypoints {
            // matches_filter is async
            let mut matches = true;
            for filter in filters {
                if !self.matches_filter(&waypoint, filter).await {
                    matches = false;
                    break;
                }
            }
            if matches {
                filtered.push(waypoint);
            }
        }
        filtered
    }

    pub async fn estimate_duration_matrix(
        &self,
        system_symbol: &SystemSymbol,
        speed: i64,
        fuel_capacity: i64,
    ) -> BTreeMap<WaypointSymbol, BTreeMap<WaypointSymbol, i64>> {
        let waypoints = self.get_system_waypoints(system_symbol).await;
        let pathfinding = Pathfinding::new(waypoints);
        pathfinding.estimate_duration_matrix(speed, fuel_capacity)
    }

    pub async fn get_route(
        &self,
        src: &WaypointSymbol,
        dest: &WaypointSymbol,
        speed: i64,
        start_fuel: i64,
        fuel_capacity: i64,
    ) -> Route {
        let system_symbol = src.system();
        assert_eq!(system_symbol, dest.system());
        let waypoints = self.get_system_waypoints(&system_symbol).await;
        let pathfinding = Pathfinding::new(waypoints);
        pathfinding.get_route(src, dest, speed, start_fuel, fuel_capacity)
    }

    // make sure factions loaded
    pub async fn load_factions(&self) {
        let db_faction_key = "factions";
        if self.factions.len() > 0 {
            return;
        }

        // Layer - check db
        let factions: Option<Vec<Faction>> = self.db.get_value(db_faction_key).await;
        if let Some(factions) = factions {
            for faction in factions {
                self.factions
                    .insert(faction.symbol.clone(), faction.clone());
            }
        }
        // Layer - fetch from api
        let factions: Vec<Faction> = self.api_client.get_all_pages("/factions").await;
        self.db.set_value(db_faction_key, &factions).await;
        for faction in factions {
            self.factions
                .insert(faction.symbol.clone(), faction.clone());
        }
    }

    pub async fn get_faction(&self, faction: &str) -> Faction {
        self.load_factions().await;
        self.factions.get(faction).unwrap().clone()
    }

    pub async fn get_jumpgate(&self, symbol: &SystemSymbol) -> WaypointSymbol {
        let waypoints = self.get_system_waypoints(symbol).await;
        waypoints
            .into_iter()
            .find(|waypoint| waypoint.is_jump_gate())
            .unwrap()
            .symbol
    }

    pub async fn get_jumpgate_connections(&self, symbol: &WaypointSymbol) -> JumpGateInfo {
        let db_jumpgate_key = format!("jumpgate/{}", symbol);

        // Layer 1 - check cache
        if let Some(jumpgate_info) = &self.jumpgates.get(symbol) {
            return jumpgate_info.value().clone();
        }
        // Layer 2 - check db
        let jumpgate_info: Option<JumpGateInfo> = self.db.get_value(&db_jumpgate_key).await;
        if let Some(jumpgate_info) = jumpgate_info {
            // !! at this point we might want to do a freshness check on jumpgate_info.timestamp if uncharted
            // in case it's been charted since we last fetched it
            self.jumpgates.insert(symbol.clone(), jumpgate_info.clone());
            return jumpgate_info;
        }
        // Layer 3 - fetch from api
        let jumpgate_info = self.api_client.get_jumpgate_conns(symbol).await;
        self.db.set_value(&db_jumpgate_key, &jumpgate_info).await;
        self.jumpgates.insert(symbol.clone(), jumpgate_info.clone());
        jumpgate_info
    }
}
