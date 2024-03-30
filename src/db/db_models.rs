use chrono::{DateTime, Utc};
use diesel::{Insertable, Queryable, QueryableByName};

#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = crate::schema::systems)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewSystem {
    pub symbol: String,
    pub type_: String,
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = crate::schema::waypoints)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewWaypoint {
    pub symbol: String,
    pub system_symbol: String,
    pub type_: String,
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Queryable, QueryableByName)]
#[diesel(table_name = crate::schema::systems)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct System {
    pub symbol: String,
    pub type_: String,
    pub x: i32,
    pub y: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Queryable, QueryableByName)]
#[diesel(table_name = crate::schema::waypoints)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Waypoint {
    pub symbol: String,
    pub system_symbol: String,
    pub type_: String,
    pub x: i32,
    pub y: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
