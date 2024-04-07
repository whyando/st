//!
//! This is a utility to delete all rows from tables in the database.
//! Must be updated when new tables are added.
//!

use diesel_async::RunQueryDsl as _;
use st::db::DbClient;
use st::schema::*;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    pretty_env_logger::init_timed();

    let db = DbClient::new("").await;
    let mut conn = db.conn().await;

    diesel::delete(general_lookup::table)
        .execute(&mut conn)
        .await
        .unwrap();
    diesel::delete(jumpgate_connections::table)
        .execute(&mut conn)
        .await
        .unwrap();
    diesel::delete(market_trades::table)
        .execute(&mut conn)
        .await
        .unwrap();
    diesel::delete(market_transactions::table)
        .execute(&mut conn)
        .await
        .unwrap();
    diesel::delete(surveys::table)
        .execute(&mut conn)
        .await
        .unwrap();
    diesel::delete(systems::table)
        .execute(&mut conn)
        .await
        .unwrap();
    diesel::delete(waypoint_details::table)
        .execute(&mut conn)
        .await
        .unwrap();
    diesel::delete(waypoints::table)
        .execute(&mut conn)
        .await
        .unwrap();
    println!("All tables truncated");
}
