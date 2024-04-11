pub mod pathfinding;

use crate::api_client::api_models;
use crate::api_client::api_models::WaypointDetailed;
use crate::api_client::ApiClient;
use crate::db::db_models;
use crate::db::db_models::NewWaypointDetails;
use crate::db::DbClient;
use crate::models::{
    Construction, Faction, Market, MarketRemoteView, Shipyard, ShipyardRemoteView, System,
    SystemSymbol, Waypoint, WaypointSymbol, WithTimestamp,
};
use crate::models::{SymbolNameDescr, WaypointDetails};
use crate::pathfinding::{Pathfinding, Route};
use crate::schema::*;
use dashmap::DashMap;
use diesel::upsert::excluded;
use diesel::BelongingToDsl as _;
use diesel::ExpressionMethods as _;
use diesel::GroupedBy as _;
use diesel::QueryDsl as _;
use diesel::SelectableHelper as _;
use diesel_async::RunQueryDsl as _;
use log::*;
use moka::future::Cache;
use std::collections::BTreeMap;
use std::sync::Arc;

use self::pathfinding::WarpEdge;

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

#[derive(Debug, Clone)]
pub struct JumpGateInfo {
    pub is_constructed: bool,
    pub connections: Vec<WaypointSymbol>,
}

pub struct Universe {
    api_client: ApiClient,
    db: DbClient,

    systems: DashMap<SystemSymbol, System>,
    constructions: DashMap<WaypointSymbol, Arc<WithTimestamp<Option<Construction>>>>,
    remote_markets: DashMap<WaypointSymbol, MarketRemoteView>,
    markets: DashMap<WaypointSymbol, Option<Arc<WithTimestamp<Market>>>>,
    remote_shipyards: DashMap<WaypointSymbol, ShipyardRemoteView>,
    shipyards: DashMap<WaypointSymbol, Option<Arc<WithTimestamp<Shipyard>>>>,
    factions: DashMap<String, Faction>,
    jumpgates: DashMap<WaypointSymbol, JumpGateInfo>,

    // cache
    warp_jump_graph: Cache<(), BTreeMap<SystemSymbol, BTreeMap<SystemSymbol, WarpEdge>>>,
}

impl Universe {
    pub fn new(api_client: &ApiClient, db: &DbClient) -> Self {
        Self {
            api_client: api_client.clone(),
            db: db.clone(),
            systems: DashMap::new(),
            constructions: DashMap::new(),
            remote_markets: DashMap::new(),
            markets: DashMap::new(),
            remote_shipyards: DashMap::new(),
            shipyards: DashMap::new(),
            factions: DashMap::new(),
            jumpgates: DashMap::new(),
            warp_jump_graph: Cache::new(1),
        }
    }

    pub async fn init(&self) {
        self.init_systems().await;
        self.init_jumpgates().await;
    }

    async fn init_systems(&self) {
        let status = self.api_client.status().await;
        let query_start = std::time::Instant::now();
        let systems: Vec<db_models::System> = systems::table
            .filter(systems::reset_id.eq(self.db.reset_date()))
            .select(db_models::System::as_select())
            .load(&mut self.db.conn().await)
            .await
            .expect("DB Query error");
        let duration = query_start.elapsed().as_millis() as f64 / 1000.0;
        info!("Loaded {} systems in {:.3}s", systems.len(), duration);

        let query_start = std::time::Instant::now();
        let waypoints = db_models::Waypoint::belonging_to(&systems)
            .select(db_models::Waypoint::as_select())
            .load(&mut self.db.conn().await)
            .await
            .expect("DB Query error");
        let duration = query_start.elapsed().as_millis() as f64 / 1000.0;
        info!("Loaded {} waypoints in {:.3}s", waypoints.len(), duration);

        let query_start = std::time::Instant::now();
        let waypoint_details = db_models::WaypointDetails::belonging_to(&waypoints)
            .select(db_models::WaypointDetails::as_select())
            .load(&mut self.db.conn().await)
            .await
            .expect("DB Query error");
        let duration = query_start.elapsed().as_millis() as f64 / 1000.0;
        info!(
            "Loaded {} waypoint details in {:.3}s",
            waypoint_details.len(),
            duration
        );

        let num_systems = systems.len() as i64;
        let grouped_details = waypoint_details.grouped_by(&waypoints);
        let waypoints = waypoints
            .into_iter()
            .zip(grouped_details)
            .grouped_by(&systems);

        let system_iter = std::iter::zip(systems, waypoints);
        if num_systems == status.stats.systems {
            for (system, waypoints) in system_iter {
                let waypoints = waypoints
                    .into_iter()
                    .map(|(waypoint, details)| {
                        let details = match details.len() {
                            0 => None,
                            1 => {
                                let details = details.into_iter().next().unwrap();
                                Some(WaypointDetails {
                                    is_under_construction: details.is_under_construction,
                                    is_market: details.is_market,
                                    is_shipyard: details.is_shipyard,
                                    is_uncharted: details.is_uncharted,
                                })
                            }
                            _ => panic!("Multiple details for waypoint"),
                        };
                        Waypoint {
                            id: waypoint.id,
                            symbol: WaypointSymbol::new(&waypoint.symbol),
                            waypoint_type: waypoint.type_,
                            x: waypoint.x as i64,
                            y: waypoint.y as i64,
                            details,
                        }
                    })
                    .collect();
                self.systems.insert(
                    SystemSymbol::new(&system.symbol),
                    System {
                        symbol: SystemSymbol::new(&system.symbol),
                        system_type: system.type_,
                        x: system.x as i64,
                        y: system.y as i64,
                        waypoints,
                    },
                );
            }
        } else {
            let systems: Vec<api_models::System> = self.api_client.get("systems.json").await;
            let system_inserts = systems
                .iter()
                .map(|system| db_models::NewSystem {
                    reset_id: self.db.reset_date(),
                    symbol: system.symbol.as_str(),
                    type_: &system.system_type,
                    x: system.x as i32,
                    y: system.y as i32,
                })
                .collect::<Vec<_>>();
            info!("Inserting {} systems", system_inserts.len());
            let mut system_ids: Vec<i64> = vec![];
            for chunk in system_inserts.chunks(1000) {
                let ids: Vec<i64> = diesel::insert_into(systems::table)
                    .values(chunk)
                    .returning(systems::id)
                    .on_conflict((systems::reset_id, systems::symbol))
                    .do_update()
                    .set((
                        // Use empty ON CONFLICT UPDATE set hack to return id
                        // yes it's a hack, and empty updates have consequences, but it's okay here
                        systems::symbol.eq(excluded(systems::symbol)),
                    ))
                    .get_results(&mut self.db.conn().await)
                    .await
                    .expect("DB Insert error");
                assert_eq!(chunk.len(), ids.len());
                system_ids.extend(ids);
            }
            assert_eq!(system_ids.len(), system_inserts.len());

            let waypoint_inserts = std::iter::zip(system_ids, systems.iter())
                .flat_map(|(system_id, system)| {
                    system
                        .waypoints
                        .iter()
                        .map(move |waypoint| db_models::NewWaypoint {
                            reset_id: self.db.reset_date(),
                            symbol: waypoint.symbol.as_str(),
                            system_id: system_id,
                            type_: waypoint.waypoint_type.as_str(),
                            x: waypoint.x as i32,
                            y: waypoint.y as i32,
                        })
                })
                .collect::<Vec<_>>();
            info!("Inserting {} waypoints", waypoint_inserts.len());
            let mut waypoint_ids: Vec<i64> = vec![];
            for chunk in waypoint_inserts.chunks(1000) {
                let ids: Vec<i64> = diesel::insert_into(waypoints::table)
                    .values(chunk)
                    .on_conflict((waypoints::reset_id, waypoints::symbol))
                    .do_update()
                    .set((
                        // as above, use empty ON CONFLICT UPDATE set hack to return id
                        // yes it's a hack, and empty updates have consequences, but it's okay here
                        waypoints::symbol.eq(excluded(waypoints::symbol)),
                    ))
                    .returning(waypoints::id)
                    .get_results(&mut self.db.conn().await)
                    .await
                    .expect("DB Insert error");
                assert_eq!(chunk.len(), ids.len());
                waypoint_ids.extend(ids);
            }
            assert_eq!(waypoint_ids.len(), waypoint_inserts.len());

            let waypoint_id_map = std::iter::zip(waypoint_ids, waypoint_inserts)
                .map(|(id, waypoint)| (waypoint.symbol.to_string(), id))
                .collect::<std::collections::HashMap<_, _>>();

            for system in systems.into_iter() {
                let system = System {
                    symbol: system.symbol.clone(),
                    system_type: system.system_type,
                    x: system.x,
                    y: system.y,
                    waypoints: system
                        .waypoints
                        .into_iter()
                        .map(|waypoint| Waypoint {
                            id: waypoint_id_map[waypoint.symbol.as_str()],
                            symbol: waypoint.symbol.clone(),
                            waypoint_type: waypoint.waypoint_type,
                            x: waypoint.x,
                            y: waypoint.y,
                            details: None,
                        })
                        .collect(),
                };
                self.systems.insert(system.symbol.clone(), system);
            }
        }
    }

    async fn init_jumpgates(&self) {
        let query_start = std::time::Instant::now();
        let jumpgates: Vec<db_models::JumpGateConnections> = jumpgate_connections::table
            .filter(jumpgate_connections::reset_id.eq(self.db.reset_date()))
            .select(db_models::JumpGateConnections::as_select())
            .load(&mut self.db.conn().await)
            .await
            .expect("DB Query error");
        let duration = query_start.elapsed().as_millis() as f64 / 1000.0;
        info!("Loaded {} jumpgates in {:.3}s", jumpgates.len(), duration);

        for jumpgate in jumpgates {
            self.jumpgates.insert(
                WaypointSymbol::new(&jumpgate.waypoint_symbol),
                JumpGateInfo {
                    is_constructed: !jumpgate.is_under_construction,
                    connections: jumpgate
                        .edges
                        .iter()
                        .map(|symbol| WaypointSymbol::new(symbol))
                        .collect(),
                },
            );
        }
    }

    pub fn connections_known(&self, waypoint: &WaypointSymbol) -> bool {
        self.jumpgates.contains_key(waypoint)
    }

    pub fn systems(&self) -> Vec<System> {
        self.systems.iter().map(|x| x.value().clone()).collect()
    }
    pub fn num_systems(&self) -> usize {
        self.systems.len()
    }
    pub fn num_waypoints(&self) -> usize {
        self.systems.iter().map(|s| s.value().waypoints.len()).sum()
    }
    pub fn system(&self, symbol: &SystemSymbol) -> System {
        self.systems
            .get(symbol)
            .expect("System not found")
            .value()
            .clone()
    }
    pub fn waypoint(&self, symbol: &WaypointSymbol) -> Waypoint {
        let system_symbol = symbol.system();
        let system = self.systems.get(&system_symbol).expect("System not found");
        system
            .value()
            .waypoints
            .iter()
            .find(|w| &w.symbol == symbol)
            .expect("Waypoint not found")
            .clone()
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
        self.systems
            .get(symbol)
            .expect("System not found")
            .value()
            .clone()
    }

    pub async fn get_system_waypoints(&self, symbol: &SystemSymbol) -> Vec<WaypointDetailed> {
        let system = self.get_system(symbol).await;
        // Collect Vec<Option<_>> to Option<Vec<_>>
        let waypoints: Option<Vec<WaypointDetailed>> = system
            .waypoints
            .iter()
            .map(|w| match &w.details {
                Some(details) => {
                    let mut traits = vec![];
                    if details.is_market {
                        traits.push("MARKETPLACE".to_string());
                    }
                    if details.is_shipyard {
                        traits.push("SHIPYARD".to_string());
                    }
                    if details.is_uncharted {
                        traits.push("UNCHARTED".to_string());
                    }
                    let traits = traits
                        .into_iter()
                        .map(|symbol| SymbolNameDescr {
                            symbol,
                            name: String::new(),
                            description: String::new(),
                        })
                        .collect();
                    Some(WaypointDetailed {
                        system_symbol: symbol.clone(),
                        symbol: w.symbol.clone(),
                        waypoint_type: w.waypoint_type.clone(),
                        x: w.x,
                        y: w.y,
                        traits: traits,
                        // faction: None,
                        is_under_construction: details.is_under_construction,
                    })
                }
                None => None,
            })
            .collect();
        match waypoints {
            Some(waypoints) => waypoints,
            None => {
                let waypoints: Vec<WaypointDetailed> =
                    self.api_client.get_system_waypoints(symbol).await;
                assert_eq!(waypoints.len(), system.waypoints.len());
                let inserts: Vec<_> = waypoints
                    .iter()
                    .map(|waypoint| {
                        let db_waypoint = system
                            .waypoints
                            .iter()
                            .find(|w| &w.symbol == &waypoint.symbol)
                            .expect("Waypoint not found");
                        NewWaypointDetails {
                            waypoint_id: db_waypoint.id,
                            reset_id: self.db.reset_date(),
                            is_market: waypoint.is_market(),
                            is_shipyard: waypoint.is_shipyard(),
                            is_uncharted: waypoint.is_uncharted(),
                            is_under_construction: waypoint.is_under_construction,
                        }
                    })
                    .collect();
                diesel::insert_into(waypoint_details::table)
                    .values(inserts)
                    .on_conflict(waypoint_details::waypoint_id)
                    .do_nothing()
                    .execute(&mut self.db.conn().await)
                    .await
                    .expect("DB Insert error");
                // load to memory (self.systems)
                let mut s = self.systems.get_mut(symbol).unwrap();
                let s = s.value_mut();
                assert_eq!(s.waypoints.len(), waypoints.len());
                for w in s.waypoints.iter_mut() {
                    let waypoint = waypoints
                        .iter()
                        .find(|w2| &w2.symbol == &w.symbol)
                        .expect("Waypoint not found");
                    w.details = Some(WaypointDetails {
                        is_market: waypoint.is_market(),
                        is_shipyard: waypoint.is_shipyard(),
                        is_uncharted: waypoint.is_uncharted(),
                        is_under_construction: waypoint.is_under_construction,
                    });
                }
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

    pub async fn detailed_waypoint(&self, symbol: &WaypointSymbol) -> WaypointDetailed {
        let system_waypoints = self.get_system_waypoints(&symbol.system()).await;
        system_waypoints
            .into_iter()
            .find(|waypoint| &waypoint.symbol == symbol)
            .unwrap()
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

    async fn matches_filter(&self, waypoint: &WaypointDetailed, filter: &WaypointFilter) -> bool {
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
    ) -> Vec<WaypointDetailed> {
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

    pub async fn get_jumpgate_opt(&self, symbol: &SystemSymbol) -> Option<WaypointSymbol> {
        let waypoints = self.get_system_waypoints(symbol).await;
        waypoints
            .into_iter()
            .find(|waypoint| waypoint.is_jump_gate())
            .map(|waypoint| waypoint.symbol)
    }

    pub async fn get_jumpgate(&self, symbol: &SystemSymbol) -> WaypointSymbol {
        self.get_jumpgate_opt(symbol)
            .await
            .expect("No jumpgate found")
    }

    pub async fn first_waypoint(&self, symbol: &SystemSymbol) -> WaypointSymbol {
        self.get_system_waypoints(&symbol).await[0].symbol.clone()
    }

    // Get jumpgate connections for a charted system
    pub async fn get_jumpgate_connections(&self, symbol: &WaypointSymbol) -> JumpGateInfo {
        if let Some(jumpgate_info) = &self.jumpgates.get(symbol) {
            return jumpgate_info.value().clone();
        }

        // Otherwise fetch from API
        let waypoint = self.detailed_waypoint(symbol).await;
        let connections = self.api_client.get_jumpgate_conns(symbol).await;
        let info = JumpGateInfo {
            is_constructed: !waypoint.is_under_construction,
            connections,
        };
        let insert = db_models::NewJumpGateConnections {
            reset_id: self.db.reset_date(),
            waypoint_symbol: symbol.as_str(),
            is_under_construction: !info.is_constructed,
            edges: info
                .connections
                .iter()
                .map(|x| x.as_str())
                .collect::<Vec<_>>(),
        };
        diesel::insert_into(jumpgate_connections::table)
            .values(&insert)
            .on_conflict((
                jumpgate_connections::reset_id,
                jumpgate_connections::waypoint_symbol,
            ))
            .do_update()
            .set((
                jumpgate_connections::is_under_construction.eq(&insert.is_under_construction),
                jumpgate_connections::edges.eq(&insert.edges),
            ))
            .execute(&mut self.db.conn().await)
            .await
            .expect("DB Insert error");
        self.jumpgates.insert(symbol.clone(), info.clone());
        info
    }
}
