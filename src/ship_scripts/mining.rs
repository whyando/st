use crate::ship_controller::ShipController;
use crate::universe::WaypointFilter;
use crate::{data::DataClient, models::*};
use log::*;
use serde::{Deserialize, Serialize};
use MiningShuttleState::*;

async fn engineered_asteroid_location(ship: &ShipController) -> WaypointSymbol {
    let waypoints = ship
        .universe
        .search_waypoints(&ship.system(), vec![WaypointFilter::EngineeredAsteroid])
        .await;
    assert!(waypoints.len() == 1);
    waypoints[0].symbol.clone()
}

pub async fn run_surveyor(ship: ShipController) {
    info!("Starting script surveyor for {}", ship.symbol());
    ship.wait_for_transit().await;

    let asteroid_location = engineered_asteroid_location(&ship).await;
    ship.goto_waypoint(&asteroid_location).await;

    loop {
        // Automatically pushes to the survey manager
        ship.survey().await;
    }
}

pub async fn run_mining_drone(ship: ShipController) {
    info!("Starting script extraction_drone for {}", ship.symbol());
    ship.wait_for_transit().await;

    let asteroid_location = engineered_asteroid_location(&ship).await;
    ship.goto_waypoint(&asteroid_location).await;

    loop {
        let should_extract = ship.cargo_space_available() > 0;
        if should_extract {
            // get survey + extract
            let survey = ship
                .agent_controller
                .survey_manager
                .get_survey(&asteroid_location)
                .await;
            let survey = match survey {
                Some(s) => s,
                None => {
                    tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
                    continue;
                }
            };
            ship.extract_survey(&survey).await;
            // !! jettison some resources?
        } else {
            // transfer goods to shuttle, and wait till completed
            debug!("Mining drone transfer initiated");
            ship.transfer_cargo().await;
            debug!("Mining drone transfer completed");
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
enum MiningShuttleState {
    Loading,
    Selling,
}

pub async fn run_shuttle(ship: ShipController, db: DataClient) {
    info!("Starting script extraction shuttle for {}", ship.symbol());
    ship.wait_for_transit().await;

    let asteroid_location = engineered_asteroid_location(&ship).await;

    let key = format!("extract_shuttle_state/{}", ship.symbol());
    let mut state: MiningShuttleState = db.get_value(&key).await.unwrap_or(Loading);

    loop {
        match state {
            Loading => {
                if ship.cargo_space_available() == 0 {
                    state = Selling;
                    db.set_value(&key, &state).await;
                    continue;
                }
                ship.goto_waypoint(&asteroid_location).await;
                ship.orbit().await;
                ship.receive_cargo().await;
            }
            Selling => {
                if ship.cargo_empty() {
                    state = Loading;
                    db.set_value(&key, &state).await;
                    continue;
                }
                dbg!(ship.cargo_map());
                panic!("Not implemented");
                // ship.goto_waypoint(&sell_location).await;
                // ship.sell_all_cargo().await;
            }
        }
    }
}
