use imageproc::drawing::*;
use imageproc::image::{Rgb, RgbImage};
use log::*;
use pathfinding::directed::dijkstra::dijkstra_all;
use quadtree_rs::{area::AreaBuilder, point::Point, Quadtree};
use st::api_client::ApiClient;
use st::db::DbClient;
use st::models::SystemSymbol;
use st::universe::Universe;
use std::cmp::max;
use std::env;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    pretty_env_logger::init_timed();

    let callsign = env::var("AGENT_CALLSIGN").expect("AGENT_CALLSIGN env var not set");

    let api_client = ApiClient::new();
    let status = api_client.status().await;
    let db = DbClient::new(&status.reset_date).await;
    let universe = Universe::new(&api_client, &db);
    universe.init().await;
    assert_eq!(status.stats.systems, universe.num_systems() as i64);
    assert_eq!(status.stats.waypoints, universe.num_waypoints() as i64);

    let systems = universe
        .systems()
        .into_iter()
        .filter(|s| !s.waypoints.is_empty())
        .collect::<Vec<_>>();
    let jump_gate_systems = systems
        .iter()
        .filter(|s| s.waypoints.iter().any(|w| w.waypoint_type == "JUMP_GATE"))
        .collect::<Vec<_>>();
    info!(
        "Warp graph on {} systems, {} jumpgates",
        systems.len(),
        jump_gate_systems.len()
    );

    // let mut jumpgates = vec![];
    // for jump_gate_system in jump_gate_systems.iter() {
    //     let waypoint_symbol = jump_gate_system
    //         .waypoints
    //         .iter()
    //         .find(|w| w.waypoint_type == "JUMP_GATE")
    //         .unwrap()
    //         .symbol
    //         .clone();
    //     let waypoint = universe.detailed_waypoint(&waypoint_symbol).await;
    //     if !waypoint.is_uncharted() {
    //         let gate = universe.get_jumpgate_connections(&waypoint_symbol).await;
    //         jumpgates.push((jump_gate_system, gate.connections, gate.is_constructed));
    //     }
    // }
    let agent = api_client.get_agent_public(&callsign).await;
    let system = universe
        .get_faction(&agent.starting_faction)
        .await
        .headquarters
        .unwrap();
    let start = universe.get_jumpgate(&system).await;

    let graph = universe.jumpgate_graph().await;

    let reachables = dijkstra_all(&start, |node| {
        graph.get(node).unwrap().active_connections.clone()
    });
    let mut reachable_systems = vec![(start.clone(), (start.clone(), 0i64))];
    for (system, distance) in &reachables {
        reachable_systems.push((system.clone(), distance.clone()));
    }
    reachable_systems.sort_by_key(|(_system, (_, d))| *d);

    info!("Constructing quadtree");
    let mut qt = Quadtree::<i64, SystemSymbol>::new_with_anchor(
        Point {
            x: -100000,
            y: -100000,
        },
        20,
    );
    for system in systems.iter() {
        qt.insert_pt(
            Point {
                x: system.x,
                y: system.y,
            },
            system.symbol.clone(),
        );
    }
    info!("Constructing quadtree done");

    // Generate system map image
    const IMAGE_SZ: i64 = 5000;
    let max_coord = systems
        .iter()
        .map(|s| max(s.x.abs(), s.y.abs()))
        .max()
        .unwrap();
    let transform = |x: i64| {
        let x = x as f64 / ((max_coord + 1) as f64) * ((IMAGE_SZ as f64) / 2.0);
        x + (IMAGE_SZ as f64) / 2.0
    };
    let mut img = RgbImage::new(IMAGE_SZ as u32, IMAGE_SZ as u32);
    for system in jump_gate_systems.iter() {
        let reachable = reachable_systems
            .iter()
            .any(|(s, _)| s.system() == system.symbol);
        let mut color = Rgb([255, 255, 255]);
        let mut size = 1;
        if reachable {
            color = Rgb([255, 255, 0]);
            size = 5;
        }
        if system.is_starter_system() {
            color = Rgb([255, 0, 0]);
            size = 5;
        }
        if system.symbol == start.system() {
            color = Rgb([255, 255, 255]);
            size = 10;
        }
        draw_filled_circle_mut(
            &mut img,
            (transform(system.x) as i32, transform(system.y) as i32),
            size,
            color,
        );

        // warp connections
        // const LINE_DIST: i64 = 800;
        // let neighbours = qt.query(
        //     AreaBuilder::default()
        //         .anchor(Point {
        //             x: system.x - LINE_DIST,
        //             y: system.y - LINE_DIST,
        //         })
        //         .dimensions((2 * LINE_DIST + 1, 2 * LINE_DIST + 1))
        //         .build()
        //         .unwrap(),
        // );
        // for pt in neighbours {
        //     let coords = pt.anchor();
        //     let distance: i64 = {
        //         let distance2 = (system.x - coords.x).pow(2) + (system.y - coords.y).pow(2);
        //         max(1, (distance2 as f64).sqrt().round() as i64)
        //     };
        //     if distance <= LINE_DIST {
        //         let color = Rgb([120, 120, 120]);
        //         draw_line_segment_mut(
        //             &mut img,
        //             (transform(system.x) as f32, transform(system.y) as f32),
        //             (transform(coords.x) as f32, transform(coords.y) as f32),
        //             color,
        //         );
        //     }
        // }
    }

    // for (jump_gate_system, conn, _is_under_construction) in jumpgates.iter() {
    //     for conn in conn.iter() {
    //         let dest_system = systems.iter().find(|s| s.symbol == conn.system()).unwrap();
    //         let color = Rgb([0, 255, 255]);
    //         draw_line_segment_mut(
    //             &mut img,
    //             (
    //                 transform(jump_gate_system.x) as f32,
    //                 transform(jump_gate_system.y) as f32,
    //             ),
    //             (
    //                 transform(dest_system.x) as f32,
    //                 transform(dest_system.y) as f32,
    //             ),
    //             color,
    //         );
    //     }
    // }
    for (symbol, (_pre, _dist)) in &reachable_systems {
        let src_system = systems
            .iter()
            .find(|s| s.symbol == symbol.system())
            .unwrap();
        let connections = graph.get(symbol).unwrap().active_connections.clone();
        for (conn, _cd) in connections.iter() {
            let dest_system = systems.iter().find(|s| s.symbol == conn.system()).unwrap();
            let color = Rgb([0, 255, 255]);
            draw_line_segment_mut(
                &mut img,
                (
                    transform(src_system.x) as f32,
                    transform(src_system.y) as f32,
                ),
                (
                    transform(dest_system.x) as f32,
                    transform(dest_system.y) as f32,
                ),
                color,
            );
        }
    }

    img.save("system_map.png").unwrap();

    // Pathfinding strategy 1:
    // - CRUISE warp only
    // - ignore fuel/markets
    // - longest edge: 800 distance (explorer has 800 max fuel)
    // note: explorer has 40 cargo, so 4000 extra fuel can be carried

    // let src = SystemSymbol::new("X1-YR70");
    // let _dest = SystemSymbol::new("X1-NS64");

    // info!("Constructing graph");
    // const MAX_HOP_DISTANCE: i64 = 800;
    // let mut graph = BTreeMap::<SystemSymbol, Vec<(SystemSymbol, i64)>>::new();
    // for system in systems.iter() {
    //     let mut neighbours = Vec::<(SystemSymbol, i64)>::new();
    //     let area = AreaBuilder::default()
    //         .anchor(Point {
    //             x: system.x - MAX_HOP_DISTANCE,
    //             y: system.y - MAX_HOP_DISTANCE,
    //         })
    //         .dimensions((2*MAX_HOP_DISTANCE + 1, 2*MAX_HOP_DISTANCE + 1))
    //         .build()
    //         .unwrap();
    //     for pt in qt.query(area) {
    //         let coords = pt.anchor();
    //         let distance: i64 = {
    //             let distance2 = (system.x - coords.x).pow(2) + (system.y - coords.y).pow(2);
    //             max(1, (distance2 as f64).sqrt().round() as i64)
    //         };
    //         if distance <= 800 {
    //             neighbours.push((pt.value_ref().clone(), distance));
    //         }
    //     }
    //     graph.insert(system.symbol.clone(), neighbours);
    // }
    // info!("Constructing graph done");

    // let (path, cost): (Vec<SystemSymbol>, i64) = dijkstra(
    //     &src,
    //     |s| graph.get(s).unwrap().iter().map(|(n, d)| (n.clone(), *d)),
    //     |s| *s == dest,
    // )
    // .expect("No path found");
    // info!("Cost: {}", cost);
    // info!("Path: {:?}", path);
}
