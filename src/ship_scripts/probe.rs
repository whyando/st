use crate::{models::WaypointSymbol, ship_controller::ShipController};
use chrono::{DateTime, Duration, Utc};
use lazy_static::lazy_static;
use log::*;
use std::ops::Add as _;

lazy_static! {
    static ref MARKET_REFRESH_INTERVAL: Duration = Duration::minutes(6);
    static ref SHIPYARD_REFRESH_INTERVAL: Duration = Duration::minutes(60);
}

pub async fn run(ship_controller: ShipController, waypoint_symbol: &WaypointSymbol) {
    info!(
        "Starting script probe for {} - {}",
        ship_controller.symbol(),
        waypoint_symbol
    );
    ship_controller.wait_for_transit().await;
    let waypoint = ship_controller.universe.get_waypoint(waypoint_symbol).await;

    ship_controller.navigate(waypoint_symbol).await;
    ship_controller.dock().await; // don't need to dock, but do so anyway to clear 'InTransit' status

    // Random sleep for a gentler startup
    let rand_start_sleep = rand::random::<u64>() % 60;
    tokio::time::sleep(tokio::time::Duration::from_secs(rand_start_sleep)).await;

    loop {
        let now = chrono::Utc::now();
        let mut next: DateTime<Utc> = now + Duration::minutes(15);
        if waypoint.is_market() {
            let market = ship_controller.universe.get_market(waypoint_symbol).await;
            let next_refresh = match market {
                Some(market) => market.timestamp.add(*MARKET_REFRESH_INTERVAL),
                None => now,
            };
            if next_refresh <= now {
                debug!("Refreshing market {}", waypoint_symbol);
                ship_controller.refresh_market().await;
            }
            next = std::cmp::min(next, next_refresh);
        }

        if waypoint.is_shipyard() {
            let shipyard = ship_controller.universe.get_shipyard(waypoint_symbol).await;
            let next_refresh = match shipyard {
                Some(market) => market.timestamp + *SHIPYARD_REFRESH_INTERVAL,
                None => now,
            };
            if next_refresh <= now {
                debug!("Refreshing shipyard {}", waypoint_symbol);
                ship_controller.refresh_shipyard().await;
            }
            next = std::cmp::min(next, next_refresh);
        }

        let sleep_duration = next - now;
        if sleep_duration > Duration::zero() {
            debug!("Sleeping for {:.3}s", sleep_duration.num_seconds() as f64);
            tokio::time::sleep(sleep_duration.to_std().unwrap()).await;
        }
    }

    // info!("Finished script probe for {}", ship_controller.symbol());
}
