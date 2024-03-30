// use st::api_client::ApiClient;
// use st::db::DbClient;
// use st::models::SystemSymbol;
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

//     let mut systems = universe.all_systems().await;
//     systems.sort_by_key(|s| {
//         usize::MAX
//             - s.waypoints
//                 .iter()
//                 .filter(|w| w.waypoint_type != "ASTEROID")
//                 .count()
//     });

//     let mut f = File::create("starter_systems.txt")?;
//     use std::io::Write as _;

//     let src = SystemSymbol::new("X1-QK86");
//     let src_system = systems.iter().find(|s| s.symbol == src).unwrap();
//     let mut systems = systems
//         .iter()
//         .filter(|s| s.is_starter_system())
//         .map(|s| {
//             let dist = src_system.distance(s);
//             (s, dist)
//         })
//         .collect::<Vec<_>>();
//     systems.sort_by_key(|(_, dist)| *dist);

//     for (system, dist) in systems {
//         // Fetching the waypoints in order to get the faction
//         let waypoints = universe.get_system_waypoints(&system.symbol).await;
//         let faction = waypoints[0].faction.as_ref().unwrap().symbol.clone();
//         writeln!(&mut f, "{}\t{}\t{}", system.symbol, dist, faction,)?;
//     }
//     Ok(())
// }
fn main() {}
