use std::sync::Arc;

use crate::{
    models::LogisticsScriptConfig, ship_controller::ShipController, tasks::LogisticTaskManager,
};
use log::*;

pub async fn run(
    ship_controller: ShipController,
    taskmanager: Arc<LogisticTaskManager>,
    config: LogisticsScriptConfig,
) {
    info!("Starting script logistics for {}", ship_controller.symbol());
    ship_controller.wait_for_transit().await;

    let ship_symbol = ship_controller.symbol();
    let system_symbol = ship_controller.system();
    assert_eq!(
        config.use_planner,
        config.planner_config.is_some(),
        "planner_config must be set if use_planner is true"
    );

    // Register the ship with the task manager
    taskmanager
        .register_ship(
            &ship_symbol,
            &system_symbol,
            &config,
            ship_controller.cargo_capacity(),
            ship_controller.engine_speed(),
            ship_controller.fuel_capacity(),
        )
        .await;

    loop {
        // Get next action from task manager
        let action = match taskmanager
            .get_next_task(&ship_symbol, &ship_controller.waypoint())
            .await
        {
            Some(action) => action,
            None => {
                info!(
                    "Ship {} was scheduled no tasks to perform. Sleeping 5-10 minutes.",
                    ship_controller.symbol()
                );
                let rand_seconds = rand::random::<u64>() % 300;
                tokio::time::sleep(tokio::time::Duration::from_secs(300 + rand_seconds)).await;
                continue;
            }
        };

        // Execute the action
        ship_controller.goto_waypoint(&action.waypoint).await;
        ship_controller.execute_action(&action.action).await;

        // Mark the action as complete
        taskmanager.complete_action(&ship_symbol, &action).await;

        info!(
            "Ship {} completed action at {}",
            ship_controller.symbol(),
            action.waypoint
        );
    }
}
