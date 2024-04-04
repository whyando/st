use st::{
    agent_controller::AgentController, api_client::ApiClient, db::DbClient, universe::Universe,
};
use std::{env, sync::Arc};

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    pretty_env_logger::init_timed();

    let callsign = env::var("AGENT_CALLSIGN")
        .expect("AGENT_CALLSIGN env var not set")
        .to_ascii_uppercase();

    let api_client = ApiClient::new();
    let status = api_client.status().await;

    // Use the reset date on the status response as a unique identifier to partition data between resets
    let db = DbClient::new(&status.reset_date).await;
    let universe = Arc::new(Universe::new(&api_client, &db));

    // Startup Phase: register if not already registered, and load agent token
    let agent_token = match db.get_agent_token(&callsign).await {
        Some(token) => token,
        None => panic!("No agent token found for callsign: {}", &callsign),
    };
    api_client.set_agent_token(&agent_token);

    let agent_controller = AgentController::new(&api_client, &db, &universe, &callsign).await;
    // let system_symbol = agent_controller.starting_system();
    let system_symbol = st::models::SystemSymbol::new("X1-JY8");

    dbg!(agent_controller.task_manager.in_progress_tasks());
    let task_list = agent_controller
        .task_manager
        .generate_task_list(&system_symbol, 10000, false, 1)
        .await;
    println!("Generated: {} tasks", task_list.len());
    for task in task_list {
        println!("{:?}", task);
        let task_details_opt = agent_controller
            .task_manager
            .get_assigned_task_status(&task.id);
        if let Some((_task, ship, ts)) = task_details_opt {
            println!("ASSIGNED TO SHIP: {} at {}", &ship, ts);
        } else {
            println!("UNASSIGNED");
        }
    }
}
