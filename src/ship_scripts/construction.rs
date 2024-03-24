//!
//! A ship dedicated to hauling resources to the construction of the jump gate.
//!
//! This script does NOT coordinate with the logistic task manager. Which means the logistics task manager
//! needs to be configured not to create construction tasks, or any task involving the construction goods.
//!
use crate::config::CONFIG;
use crate::models::MarketActivity::*;
use crate::models::MarketSupply::*;
use crate::models::MarketType::*;
use crate::{
    data::DataClient,
    models::{Construction, WaypointSymbol},
    ship_controller::ShipController,
    universe::WaypointFilter,
};
use log::*;
use serde::{Deserialize, Serialize};
use std::cmp::min;
use ConstructionHaulerState::*;

pub async fn get_export_market(ship: &ShipController, good: &str) -> WaypointSymbol {
    let filters = vec![WaypointFilter::Exports(good.to_string())];
    let waypoints = ship
        .universe
        .search_waypoints(&ship.system(), &filters)
        .await;
    assert!(waypoints.len() == 1);
    waypoints[0].symbol.clone()
}

pub async fn get_jump_gate(ship: &ShipController) -> WaypointSymbol {
    let waypoints = ship
        .universe
        .search_waypoints(&ship.system(), &vec![WaypointFilter::JumpGate])
        .await;
    assert!(waypoints.len() == 1);
    waypoints[0].symbol.clone()
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
enum ConstructionHaulerState {
    Buying,
    Delivering,
    Completed,
}

pub async fn run_hauler(ship: ShipController, db: DataClient) {
    info!("Starting script construction_hauler for {}", ship.symbol());
    ship.wait_for_transit().await;

    let jump_gate_symbol = get_jump_gate(&ship).await;
    let fab_mat_market = get_export_market(&ship, "FAB_MATS").await;
    let adv_circuit_market = get_export_market(&ship, "ADVANCED_CIRCUITRY").await;

    let key = format!("construction_state/{}", ship.symbol());
    let mut state: ConstructionHaulerState = db.get_value(&key).await.unwrap_or(Buying);

    while state != Completed {
        let next_state = tick(
            &ship,
            state,
            &jump_gate_symbol,
            &fab_mat_market,
            &adv_circuit_market,
        )
        .await;
        if let Some(next_state) = next_state {
            state = next_state;
            db.set_value(&key, &state).await;
        }
    }
}

async fn tick(
    ship: &ShipController,
    state: ConstructionHaulerState,
    jump_gate_symbol: &WaypointSymbol,
    fab_mat_market: &WaypointSymbol,
    adv_circuit_market: &WaypointSymbol,
) -> Option<ConstructionHaulerState> {
    let construction = ship.universe.get_construction(&jump_gate_symbol).await;
    let construction: &Construction = match &construction.data {
        None => return Some(Completed),
        Some(x) if x.is_complete => return Some(Completed),
        Some(x) => x,
    };
    match state {
        Buying => {
            if ship.cargo_space_available() == 0 {
                return Some(Delivering);
            }

            // load up on construction goods
            let mut incomplete_materials = 0;
            for mat in &construction.materials {
                let holding = ship.cargo_good_count(&mat.trade_symbol);
                if mat.fulfilled + holding >= mat.required {
                    continue;
                }
                incomplete_materials += 1;
                let market_symbol = match mat.trade_symbol.as_str() {
                    "FAB_MATS" => &fab_mat_market,
                    "ADVANCED_CIRCUITRY" => &adv_circuit_market,
                    _ => panic!("Unknown construction good: {}", mat.trade_symbol),
                };
                // Add a credit buffer against advanced circuitry, since FABMATs are higher priority when credits are low
                // because they are the long pole
                let credit_buffer = match mat.trade_symbol.as_str() {
                    "FAB_MATS" => 0,
                    "ADVANCED_CIRCUITRY" => 2_000_000,
                    _ => panic!("Unknown construction good: {}", mat.trade_symbol),
                };
                let market = ship.universe.get_market(&market_symbol).await;
                if let Some(market) = market {
                    let good = market
                        .data
                        .trade_goods
                        .iter()
                        .find(|x| x.symbol == mat.trade_symbol)
                        .unwrap();
                    assert_eq!(good._type, Export);
                    let should_buy = match good.activity.as_ref().unwrap() {
                        Strong => good.supply >= High,
                        _ => good.supply >= Moderate,
                    };
                    if should_buy || CONFIG.override_construction_supply_check {
                        let required_units = mat.required - holding - mat.fulfilled;
                        let units = min(
                            good.trade_volume,
                            min(ship.cargo_space_available(), required_units),
                        );
                        ship.goto_waypoint(&market_symbol).await;

                        let expected_cost = good.purchase_price * units;
                        let credits = ship.agent_controller.ledger.available_credits();
                        if expected_cost > credits - credit_buffer {
                            debug!(
                                "Insufficient funds to buy {} units of {}. {}/{} (buffer: {})",
                                units, good.symbol, credits, expected_cost, credit_buffer
                            );
                            tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
                            return None;
                        }
                        ship.buy_goods(&good.symbol, units, false).await;
                        return None;
                    }
                }
            }
            // cargo not full and nothing to buy, retry in 60 seconds
            if incomplete_materials == 0 || ship.cargo_space_available() <= 10 {
                return Some(Delivering);
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
            return None;
        }
        Delivering => {
            if ship.cargo_empty() {
                return Some(Buying);
            }
            // todo - handle case where materials are no longer needed
            ship.goto_waypoint(&jump_gate_symbol).await;
            while let Some(cargo_item) = ship.cargo_first_item() {
                ship.supply_construction(&cargo_item.symbol, cargo_item.units)
                    .await;
            }
            None
        }
        Completed => {
            panic!("Invalid state");
        }
    }
}
