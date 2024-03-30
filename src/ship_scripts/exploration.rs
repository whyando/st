use crate::{db::DbClient, ship_controller::ShipController};
use log::*;
use serde::{Deserialize, Serialize};
use ExplorerState::*;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
enum ExplorerState {
    Init,
    Exploring,
    Exit,
}

pub async fn run_explorer(ship: ShipController, db: DbClient) {
    info!("Starting script explorer for {}", ship.symbol());
    ship.wait_for_transit().await;

    let key = format!("explorer_state/{}", ship.symbol());
    let mut state: ExplorerState = db.get_value(&key).await.unwrap_or(Init);

    while state != Exit {
        let next_state = tick(&ship, state).await;
        if let Some(next_state) = next_state {
            state = next_state;
            db.set_value(&key, &state).await;
        }
    }
}

async fn tick(_ship: &ShipController, state: ExplorerState) -> Option<ExplorerState> {
    match state {
        Init => {
            // allocate a target system to explore
            todo!();
        }
        Exploring => {
            // goto system, chart/scan until done
            // mark as completed
            todo!();
        }
        Exit => {
            panic!("Invalid state");
        }
    }
}
