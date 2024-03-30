// use st::api_client::ApiClient;
// use st::db::DbClient;
// use st::universe::Universe;
// use std::fs::File;
// use std::io;

// #[tokio::main]
// async fn main() -> io::Result<()> {
//     dotenvy::dotenv().ok();
//     pretty_env_logger::init_timed();

//     let api_client = ApiClient::new();
//     let status = api_client.status().await;

//     // Use the reset date on the status response as a unique identifier to partition data between resets
//     let db = DbClient::new(&status.reset_date).await;
//     let universe = Universe::new(&api_client, &db);

//     let mut all_systems = universe.all_systems().await;

//     all_systems.sort_by_cached_key(|s| {
//         -(s.waypoints
//             .iter()
//             .filter(|w| w.waypoint_type() != "ASTEROID")
//             .count() as i32)
//     });

//     // output to ./systems.txt
//     let mut f = File::create("systems.txt")?;
//     use std::io::Write as _;
//     for s in all_systems {
//         let num_non_asteroid = s
//             .waypoints
//             .iter()
//             .filter(|w| w.waypoint_type() != "ASTEROID")
//             .count();
//         writeln!(
//             &mut f,
//             "{}\t{}\t{} ({})",
//             s.symbol,
//             s.system_type,
//             s.waypoints.len(),
//             num_non_asteroid
//         )?;

//         if num_non_asteroid >= 10 {
//             // 20 {
//             let waypoints = universe.get_system_waypoints(&s.symbol).await;
//             let jumpgate = waypoints.iter().find(|w| w.is_jump_gate());
//             if let Some(jumpgate) = jumpgate {
//                 let construction = if jumpgate.is_under_construction {
//                     " (under construction)"
//                 } else {
//                     ""
//                 };
//                 writeln!(&mut f, "   Jumpgate: {} {}", jumpgate.symbol, construction)?;
//             }
//             let shipyards = waypoints
//                 .iter()
//                 .filter(|w| w.is_shipyard())
//                 .map(|w| w.symbol.clone())
//                 .collect::<Vec<_>>();
//             for shipyard in shipyards {
//                 let shipyard = universe.get_shipyard_remote(&shipyard).await;
//                 let ship_types = shipyard
//                     .ship_types
//                     .iter()
//                     .map(|s| s.ship_type.clone())
//                     .collect::<Vec<_>>()
//                     .join(", ");
//                 writeln!(&mut f, "   Shipyard: {} - {}", shipyard.symbol, ship_types)?;
//             }
//         }
//     }

//     Ok(())
// }
fn main() {}
