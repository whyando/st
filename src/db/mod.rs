pub mod db_models;

use crate::logistics_planner::Task;
use crate::models::Construction;
use crate::models::KeyedSurvey;
use crate::schema::*;
use crate::{
    logistics_planner::ShipSchedule,
    models::{
        Market, MarketRemoteView, Shipyard, ShipyardRemoteView, SystemSymbol, WaypointSymbol,
        WithTimestamp,
    },
};
use chrono::DateTime;
use chrono::Utc;
use dashmap::DashMap;
use diesel::sql_types::Integer;
use diesel::ExpressionMethods as _;
use diesel::OptionalExtension as _;
use diesel::QueryDsl as _;
use diesel::QueryableByName;
use diesel::SelectableHelper as _;
use diesel_async::pooled_connection::deadpool::Object;
use diesel_async::pooled_connection::deadpool::Pool;
use diesel_async::pooled_connection::AsyncDieselConnectionManager;
use diesel_async::AsyncPgConnection;
use diesel_async::RunQueryDsl as _;
use log::*;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct DbClient {
    db: Pool<AsyncPgConnection>,
    reset_id: Arc<String>,
}

impl DbClient {
    pub async fn new(reset_identifier: &str) -> DbClient {
        let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
        let db = {
            let manager = AsyncDieselConnectionManager::<AsyncPgConnection>::new(database_url);
            Pool::builder(manager).max_size(5).build().unwrap()
        };
        // Check the connection
        {
            let mut conn = db.get().await.unwrap();
            #[derive(QueryableByName)]
            struct Ret {
                #[diesel(sql_type = Integer)]
                value: i32,
            }
            let result: Vec<Ret> = diesel::sql_query("SELECT 1 as value")
                .load(&mut conn)
                .await
                .unwrap();
            assert_eq!(result.len(), 1);
            assert_eq!(result[0].value, 1);
            info!("Successfully connected to database");
        }
        DbClient {
            db,
            reset_id: Arc::new(reset_identifier.to_string()),
        }
    }

    pub fn reset_date(&self) -> &str {
        self.reset_id.as_str()
    }

    pub async fn conn(&self) -> Object<AsyncPgConnection> {
        self.db
            .get()
            .await
            .expect("Timed out waiting for a database connection")
    }

    pub async fn get_value<T>(&self, key: &str) -> Option<T>
    where
        T: Sized + DeserializeOwned,
    {
        debug!("db get: {}", key);
        let value_opt: Option<Value> = general_lookup::table
            .select(general_lookup::value)
            .filter(general_lookup::reset_id.eq(self.reset_date()))
            .filter(general_lookup::key.eq(key))
            .first(&mut self.conn().await)
            .await
            .optional()
            .expect("DB Query error");
        value_opt.map(|data| serde_json::from_value(data).unwrap())
    }

    pub async fn set_value<T>(&self, key: &str, value: &T)
    where
        T: Serialize + ?Sized,
    {
        debug!("db set: {}", key);
        let value: Value = serde_json::to_value(value).unwrap();
        diesel::insert_into(general_lookup::table)
            .values((
                general_lookup::reset_id.eq(self.reset_date()),
                general_lookup::key.eq(key),
                general_lookup::value.eq(&value),
            ))
            .on_conflict((general_lookup::reset_id, general_lookup::key))
            .do_update()
            .set(general_lookup::value.eq(&value))
            .execute(&mut self.conn().await)
            .await
            .expect("DB Query error");
    }

    pub async fn get_agent_token(&self, callsign: &str) -> Option<String> {
        self.get_value(&format!("registrations/{}", callsign)).await
    }

    pub async fn save_agent_token(&self, callsign: &str, token: &str) {
        self.set_value(&format!("registrations/{}", callsign), token)
            .await
    }

    // pub async fn get_system(&self, symbol: &SystemSymbol) -> Option<System> {
    //     self.get_value(&format!("systems/{}", symbol)).await
    // }

    // pub async fn save_system(&self, symbol: &SystemSymbol, system: &System) {
    //     self.set_value(&format!("systems/{}", symbol), system).await
    // }

    // pub async fn get_system_waypoints(&self, symbol: &SystemSymbol) -> Option<Vec<Waypoint>> {
    //     let key = format!("system_waypoints_2/{}", symbol);
    //     self.get_value(&key).await
    // }

    // pub async fn save_system_waypoints(&self, symbol: &SystemSymbol, waypoints: &Vec<Waypoint>) {
    //     let key = format!("system_waypoints_2/{}", symbol);
    //     self.set_value(&key, waypoints).await
    // }

    pub async fn get_market_remote(&self, symbol: &WaypointSymbol) -> Option<MarketRemoteView> {
        self.get_value(&format!("markets_remote/{}", symbol)).await
    }

    pub async fn save_market_remote(&self, symbol: &WaypointSymbol, market: &MarketRemoteView) {
        let key = format!("markets_remote/{}", symbol);
        self.set_value(&key, market).await
    }

    pub async fn get_shipyard_remote(&self, symbol: &WaypointSymbol) -> Option<ShipyardRemoteView> {
        let key = format!("shipyards_remote/{}", symbol);
        self.get_value(&key).await
    }

    pub async fn save_shipyard_remote(
        &self,
        symbol: &WaypointSymbol,
        shipyard: &ShipyardRemoteView,
    ) {
        let key = format!("shipyards_remote/{}", symbol);
        self.set_value(&key, shipyard).await
    }

    pub async fn get_market(&self, symbol: &WaypointSymbol) -> Option<WithTimestamp<Market>> {
        let key = format!("markets/{}", symbol);
        self.get_value(&key).await
    }

    pub async fn save_market(&self, symbol: &WaypointSymbol, market: &WithTimestamp<Market>) {
        // save to snapshot market view
        let key = format!("markets/{}", symbol);
        self.set_value(&key, &market).await;
    }

    pub async fn insert_market_trades(&self, market: &WithTimestamp<Market>) {
        let inserts = market
            .data
            .trade_goods
            .iter()
            .map(|trade| {
                let activity = trade.activity.as_ref().map(|a| a.to_string());
                (
                    market_trades::timestamp.eq(market.timestamp),
                    market_trades::market_symbol.eq(market.data.symbol.to_string()),
                    market_trades::symbol.eq(&trade.symbol),
                    market_trades::trade_volume.eq(trade.trade_volume as i32),
                    market_trades::type_.eq(trade._type.to_string()),
                    market_trades::supply.eq(trade.supply.to_string()),
                    market_trades::activity.eq(activity),
                    market_trades::purchase_price.eq(trade.purchase_price as i32),
                    market_trades::sell_price.eq(trade.sell_price as i32),
                )
            })
            .collect::<Vec<_>>();
        diesel::insert_into(market_trades::table)
            .values(&inserts)
            .execute(&mut self.conn().await)
            .await
            .expect("DB Query error");
    }

    pub async fn upsert_market_transactions(&self, market: &WithTimestamp<Market>) {
        let inserts = market
            .data
            .transactions
            .iter()
            .map(|transaction| {
                (
                    market_transactions::timestamp.eq(transaction.timestamp),
                    market_transactions::market_symbol.eq(market.data.symbol.to_string()),
                    market_transactions::symbol.eq(&transaction.trade_symbol),
                    market_transactions::ship_symbol.eq(&transaction.ship_symbol),
                    market_transactions::type_.eq(&transaction._type),
                    market_transactions::units.eq(transaction.units as i32),
                    market_transactions::price_per_unit.eq(transaction.price_per_unit as i32),
                    market_transactions::total_price.eq(transaction.total_price as i32),
                )
            })
            .collect::<Vec<_>>();
        diesel::insert_into(market_transactions::table)
            .values(inserts)
            .on_conflict((
                market_transactions::market_symbol,
                market_transactions::timestamp,
            ))
            .do_nothing()
            .execute(&mut self.conn().await)
            .await
            .expect("DB Query error");
    }

    pub async fn get_shipyard(&self, symbol: &WaypointSymbol) -> Option<WithTimestamp<Shipyard>> {
        let key = format!("shipyards/{}", symbol);
        self.get_value(&key).await
    }

    pub async fn save_shipyard(&self, symbol: &WaypointSymbol, shipyard: &WithTimestamp<Shipyard>) {
        let key = format!("shipyards/{}", symbol);
        self.set_value(&key, &shipyard).await;
    }

    pub async fn load_schedule(&self, ship_symbol: &str) -> Option<ShipSchedule> {
        let key = format!("schedules/{}", ship_symbol);
        self.get_value(&key).await
    }
    pub async fn load_schedule_progress(&self, ship_symbol: &str) -> Option<usize> {
        let key = format!("schedule_progress/{}", ship_symbol);
        self.get_value(&key).await
    }
    pub async fn save_schedule(&self, ship_symbol: &str, schedule: &ShipSchedule) {
        let key = format!("schedules/{}", ship_symbol);
        self.set_value(&key, schedule).await
    }
    pub async fn save_schedule_progress(&self, ship_symbol: &str, progress: usize) {
        let key = format!("schedule_progress/{}", ship_symbol);
        self.set_value(&key, &progress).await
    }
    pub async fn update_schedule_progress(&self, ship_symbol: &str, progress: usize) {
        self.save_schedule_progress(ship_symbol, progress).await;
    }

    // type TaskManagerStatus = DashMap<String, (Task, String, DateTime<Utc>)>
    pub async fn save_task_manager_state(
        &self,
        system_symbol: &SystemSymbol,
        status: &DashMap<String, (Task, String, DateTime<Utc>)>,
    ) {
        let key = format!("task_manager/{}", system_symbol);
        self.set_value(&key, status).await
    }
    pub async fn load_task_manager_state(
        &self,
        system_symbol: &SystemSymbol,
    ) -> Option<DashMap<String, (Task, String, DateTime<Utc>)>> {
        let key = format!("task_manager/{}", system_symbol);
        self.get_value(&key).await
    }

    pub async fn get_construction(
        &self,
        symbol: &WaypointSymbol,
    ) -> Option<WithTimestamp<Option<Construction>>> {
        let key = format!("construction/{}", symbol);
        self.get_value(&key).await
    }
    pub async fn save_construction(
        &self,
        symbol: &WaypointSymbol,
        construction: &WithTimestamp<Option<Construction>>,
    ) {
        let key = format!("construction/{}", symbol);
        self.set_value(&key, construction).await
    }

    pub async fn get_probe_jumpgate_reservations(
        &self,
        callsign: &str,
    ) -> DashMap<String, WaypointSymbol> {
        let key = format!("probe_jumpgate_reservations/{}", callsign);
        self.get_value(&key).await.unwrap_or_default()
    }

    pub async fn save_probe_jumpgate_reservations(
        &self,
        callsign: &str,
        reservations: &DashMap<String, WaypointSymbol>,
    ) {
        let key = format!("probe_jumpgate_reservations/{}", callsign);
        self.set_value(&key, &reservations).await
    }

    pub async fn get_explorer_reservations(&self, callsign: &str) -> DashMap<String, SystemSymbol> {
        let key = format!("explorer_reservations/{}", callsign);
        self.get_value(&key).await.unwrap_or_default()
    }

    pub async fn save_explorer_reservations(
        &self,
        callsign: &str,
        reservations: &DashMap<String, SystemSymbol>,
    ) {
        let key = format!("explorer_reservations/{}", callsign);
        self.set_value(&key, &reservations).await
    }

    pub async fn insert_surveys(&self, surveys: &Vec<KeyedSurvey>) {
        let now = Utc::now();
        let inserts = surveys
            .iter()
            .map(|survey| {
                (
                    surveys::reset_id.eq(self.reset_date()),
                    surveys::uuid.eq(&survey.uuid),
                    surveys::survey.eq(serde_json::to_value(&survey.survey).unwrap()),
                    surveys::asteroid_symbol.eq(survey.survey.symbol.to_string()),
                    surveys::inserted_at.eq(now),
                    surveys::expires_at.eq(survey.survey.expiration),
                )
            })
            .collect::<Vec<_>>();
        diesel::insert_into(surveys::table)
            .values(&inserts)
            .execute(&mut self.conn().await)
            .await
            .expect("DB Query error");
    }

    pub async fn get_surveys(&self) -> Vec<KeyedSurvey> {
        let surveys: Vec<(Uuid, Value)> = surveys::table
            .filter(surveys::reset_id.eq(self.reset_date()))
            .select((surveys::uuid, surveys::survey))
            .load(&mut self.conn().await)
            .await
            .expect("DB Query error");
        surveys
            .into_iter()
            .map(|(uuid, survey)| KeyedSurvey {
                uuid,
                survey: serde_json::from_value(survey).unwrap(),
            })
            .collect()
    }

    pub async fn remove_survey(&self, uuid: &Uuid) {
        diesel::delete(
            surveys::table
                .filter(surveys::reset_id.eq(self.reset_date()))
                .filter(surveys::uuid.eq(uuid)),
        )
        .execute(&mut self.conn().await)
        .await
        .expect("DB Query error");
    }

    pub async fn get_systems(&self) -> Vec<db_models::System> {
        systems::table
            .filter(systems::reset_id.eq(self.reset_date()))
            .select(db_models::System::as_select())
            .load(&mut self.conn().await)
            .await
            .expect("DB Query error")
    }

    pub async fn insert_systems(&self, systems: &Vec<db_models::NewSystem<'_>>) {
        diesel::insert_into(systems::table)
            .values(systems)
            .execute(&mut self.conn().await)
            .await
            .expect("DB Query error");
    }
}
