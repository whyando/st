use chrono::{DateTime, Utc};
use diesel::{
    associations::Associations, Identifiable, Insertable, Queryable, QueryableByName, Selectable,
};

#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = crate::schema::systems)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewSystem<'a> {
    pub reset_id: &'a str,
    pub symbol: &'a str,
    pub type_: &'a str,
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = crate::schema::waypoints)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewWaypoint<'a> {
    pub reset_id: &'a str,
    pub symbol: &'a str,
    pub system_id: i64,
    pub type_: &'a str,
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = crate::schema::waypoint_details)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewWaypointDetails<'a> {
    pub reset_id: &'a str,
    pub waypoint_id: i64,
    pub is_market: bool,
    pub is_shipyard: bool,
    pub is_uncharted: bool,
    pub is_under_construction: bool,
}

#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = crate::schema::jumpgate_connections)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewJumpgateConnections<'a> {
    pub reset_id: &'a str,
    pub waypoint_symbol: &'a str,
    pub edges: Vec<&'a str>,
}

#[derive(Debug, Clone, Identifiable, Queryable, QueryableByName, Selectable)]
#[diesel(table_name = crate::schema::systems)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct System {
    pub id: i64,
    pub symbol: String,
    pub type_: String,
    pub x: i32,
    pub y: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Identifiable, Queryable, QueryableByName, Selectable, Associations)]
#[diesel(belongs_to(System))]
#[diesel(table_name = crate::schema::waypoints)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Waypoint {
    pub id: i64,
    pub symbol: String,
    pub system_id: i64,
    pub type_: String,
    pub x: i32,
    pub y: i32,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Identifiable, Queryable, QueryableByName, Selectable, Associations)]
#[diesel(belongs_to(Waypoint))]
#[diesel(table_name = crate::schema::waypoint_details)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct WaypointDetails {
    pub id: i64,
    pub waypoint_id: i64,
    pub is_market: bool,
    pub is_shipyard: bool,
    pub is_uncharted: bool,
    pub is_under_construction: bool,
}

#[derive(Debug, Clone, Queryable, QueryableByName, Selectable)]
#[diesel(table_name = crate::schema::jumpgate_connections)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct JumpGateConnections {
    pub waypoint_symbol: String,
    pub is_under_construction: bool,
    pub edges: Vec<String>,
}
