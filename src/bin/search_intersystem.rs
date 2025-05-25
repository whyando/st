// use tracing::*;
// use st::api_client::ApiClient;
// use st::db::DbClient;
// use st::models::WaypointSymbol;
// use st::universe::Universe;

// #[tokio::main]
// async fn main() {
//     dotenvy::dotenv().ok();
//     pretty_env_logger::init_timed();

//     let api_client = ApiClient::new();
//     let status = api_client.status().await;

//     // Use the reset date on the status response as a unique identifier to partition data between resets
//     let db = DbClient::new(&status.reset_date).await;
//     let universe = Universe::new(&api_client, &db);

//     let mut systems = universe.all_systems().await;
//     info!("Systems: {}", systems.len());

//     systems.sort_by_key(|s| {
//         usize::MAX
//             - s.waypoints
//                 .iter()
//                 .filter(|w| w.waypoint_type != "ASTEROID")
//                 .count()
//     });
//     for system in systems.iter().take(500) {
//         let waypoints = universe.get_system_waypoints(&system.symbol).await;
//         let num_non_asteroid = system
//             .waypoints
//             .iter()
//             .filter(|w| w.waypoint_type != "ASTEROID")
//             .count();
//         let num_en_asteroid = system
//             .waypoints
//             .iter()
//             .filter(|w| w.waypoint_type == "ENGINEERED_ASTEROID")
//             .count();
//         let shipyards = waypoints.iter().filter(|w| w.is_shipyard()).count();
//         let markets = waypoints.iter().filter(|w| w.is_market()).count();
//         info!(
//             "{} {} (+{})",
//             system.symbol,
//             num_non_asteroid,
//             (system.waypoints.len() - num_non_asteroid)
//         );
//         info!("  M {}, SY {}, EA: {}", markets, shipyards, num_en_asteroid);
//         if num_en_asteroid == 1 {
//             let a1 = WaypointSymbol::new(&format!("{}-A1", system.symbol));
//             let a4 = WaypointSymbol::new(&format!("{}-A4", system.symbol));
//             let a1 = universe.get_market_remote(&a1).await;
//             let a4 = universe.get_market_remote(&a4).await;
//             let imports = a1
//                 .imports
//                 .iter()
//                 .map(|i| i.symbol.clone())
//                 .collect::<Vec<String>>()
//                 .join(", ");
//             let exports = a4
//                 .exports
//                 .iter()
//                 .map(|i| i.symbol.clone())
//                 .collect::<Vec<String>>()
//                 .join(", ");
//             info!("  IMPORTS: {}", imports);
//             info!("  EXPORTS: {}", exports);
//         }
//     }
// }
fn main() {}
