// https://reinterpretcat.github.io/vrp/concepts/pragmatic/problem/jobs.html

use std::collections::BTreeSet;
use std::sync::Arc;

use chrono::{DateTime, NaiveDateTime, Utc};
use serde_json::json;
use st::api_client::api_models::WaypointDetailed;
use st::models::{Market, MarketTradeGood, PaginatedList, WaypointSymbol};

use vrp_pragmatic::checker::CheckerContext;
use vrp_pragmatic::core::models::{Problem as CoreProblem, Solution as CoreSolution};
use vrp_pragmatic::core::prelude::*;
use vrp_pragmatic::core::solver::VrpConfigBuilder;
use vrp_pragmatic::format::problem::{Matrix, PragmaticProblem, Problem};
use vrp_pragmatic::format::solution::{deserialize_solution, write_pragmatic, Solution};
use vrp_pragmatic::format::Location;

fn waypoint_index(locations: &mut Vec<WaypointSymbol>, symbol: &WaypointSymbol) -> usize {
    match locations.iter().position(|s| s == symbol) {
        Some(index) => index,
        None => {
            locations.push(symbol.clone());
            locations.len() - 1
        }
    }
}

// return timestamp in RFC3339 format of seconds since 0000-01-01T00:00:00Z
fn get_timestamp(seconds: i64) -> String {
    #[allow(deprecated)]
    let dt = DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(seconds, 0), Utc);
    // rfc3339
    dt.to_rfc3339()
}

fn from_timestamp(timestamp: &str) -> i64 {
    DateTime::parse_from_rfc3339(timestamp).unwrap().timestamp()
}

fn main() {
    // sample data snapshotted from another agent
    let dir = "./test_data/2024-01-28/100L-TRADER2";

    // load all files with prefix local-market
    let mut markets = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("local-market")
        {
            let market: Market =
                serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
            markets.push(market);
        }
    }
    println!("{} markets loaded", markets.len());

    // load market waypoints from ./test_data/waypoints_page_1.json and ./test_data/waypoints_page_2.json
    let waypoints = {
        let waypoints1: PaginatedList<WaypointDetailed> = serde_json::from_str(
            &std::fs::read_to_string("./test_data/waypoints_page_1.json").unwrap(),
        )
        .unwrap();
        let waypoints2: PaginatedList<WaypointDetailed> = serde_json::from_str(
            &std::fs::read_to_string("./test_data/waypoints_page_2.json").unwrap(),
        )
        .unwrap();
        let mut waypoints = waypoints1.data;
        waypoints.extend(waypoints2.data);
        waypoints
    };

    // unique list of goods
    let mut goods = Vec::new();
    for market in markets.iter() {
        for trade in market.trade_goods.iter() {
            if !goods.contains(&trade.symbol) {
                goods.push(trade.symbol.clone());
            }
        }
    }
    println!("{} goods loaded", goods.len());
    println!("{:?}", goods);

    // let NUM_VEHICLES = 1;
    // let CAPACITY = 40;
    // let SPEED = 30.0;
    #[allow(non_snake_case)]
    let NUM_VEHICLES = 2;
    #[allow(non_snake_case)]
    let CAPACITY = 80;
    #[allow(non_snake_case)]
    let SPEED = 10.0;
    #[allow(non_snake_case)]
    let TIME_LIMIT_MINUTES = 30;
    #[allow(non_snake_case)]
    let START_WAYPOINT = waypoints[0].symbol.clone();

    // ?! Add only 1 job for each good
    // we can't add multiple jobs for a (market, good) pair, because the act of completing one job will change the reward for the next job

    let mut jobs = Vec::new();
    let mut locations = Vec::new();
    for good in goods.iter() {
        // filter + flatten to trades for this good
        let trades: Vec<(WaypointSymbol, MarketTradeGood)> = markets
            .iter()
            .filter_map(|m| {
                m.trade_goods
                    .iter()
                    .find(|g| g.symbol == *good)
                    .map(|g| (m.symbol.clone(), g.clone()))
            })
            .collect();
        let buy_trade_good = trades
            .iter()
            .min_by_key(|(_, trade)| trade.purchase_price)
            .unwrap();
        let sell_trade_good = trades
            .iter()
            .max_by_key(|(_, trade)| trade.sell_price)
            .unwrap();
        let units = std::cmp::min(
            std::cmp::min(
                buy_trade_good.1.trade_volume,
                sell_trade_good.1.trade_volume,
            ),
            CAPACITY,
        );
        let profit =
            (sell_trade_good.1.sell_price - buy_trade_good.1.purchase_price) * (units as i64);
        if profit > 0 {
            println!(
                "{}: buy {} @ {} for ${}, sell @ {} for ${}, profit: ${}",
                good,
                units,
                buy_trade_good.0,
                buy_trade_good.1.purchase_price,
                sell_trade_good.0,
                sell_trade_good.1.sell_price,
                profit
            );
            let job = json!({
                "id": good,
                "pickups": [{
                    "places": [{
                        "tag": format!("BUY {} {} @ {} for ${}", units, good, buy_trade_good.0, buy_trade_good.1.purchase_price),
                        "location": {
                            "index": waypoint_index(&mut locations, &buy_trade_good.0),
                        },
                        "times": [[get_timestamp(0), get_timestamp(TIME_LIMIT_MINUTES * 60)]],
                        "duration": 0,
                    }],
                    "demand": [units],
                    //"order": 100000 - profit,
                }],
                "deliveries": [{
                    "places": [{
                        "tag": format!("SELL {} {} @ {} for ${} (profit: ${})", units, good, sell_trade_good.0, sell_trade_good.1.sell_price, profit),
                        "location": {
                            "index": waypoint_index(&mut locations, &sell_trade_good.0),
                        },
                        "times": [[get_timestamp(0), get_timestamp(TIME_LIMIT_MINUTES * 60)]],
                        "duration": 0,
                    }],
                    "demand": [units],
                    //"order": 100000 - profit,
                }],
                "value": profit,
            });
            jobs.push(job);
        }
    }

    println!("{} jobs loaded", jobs.len());

    let vehicle_ids = (1..=NUM_VEHICLES)
        .map(|i| format!("TRADER-{}", i))
        .collect::<Vec<String>>();
    let vehicles = json!([{
        "typeId": "TRADER",
        "vehicleIds": vehicle_ids,
        "profile": {
            "matrix": "matrix1",
            "scale": 1.0,
        },
        "costs": {
            "fixed": 0,
            "distance": 0.0001,
            "time": 0.0001,
        },
        "shifts": [{
            "start": {
                "earliest": get_timestamp(0),
                "location": {
                    "index": waypoint_index(&mut locations, &START_WAYPOINT),
                },
            },
            // "end": {
            //     "latest": get_timestamp(TIME_LIMIT_MINUTES * 60),
            //     // doesn't make sense to me since we need to set the time limit, but no end location
            //     "location": {
            //         "index": waypoint_index(&mut locations, &START_WAYPOINT),
            //     },
            // },
        }],
        "capacity": [CAPACITY],
    }]);

    let problem: Problem = serde_json::from_value(json!({
        "plan": {
            "jobs": jobs,
        },
        "fleet": {
            "vehicles": vehicles,
            "profiles": [{ "name": "matrix1", }],
        },
    }))
    .unwrap();

    for (i, location) in locations.iter().enumerate() {
        println!("{}: {}", i, location);
    }

    let mut travel_times = vec![];
    let mut distances = vec![];
    for src_symbol in locations.iter() {
        for dest_symbol in locations.iter() {
            let src = waypoints.iter().find(|w| w.symbol == *src_symbol).unwrap();
            let dest = waypoints.iter().find(|w| w.symbol == *dest_symbol).unwrap();
            // euclidean distance
            let d2 = (src.x - dest.x).pow(2) + (src.y - dest.y).pow(2);
            let d = (d2 as f64).sqrt();
            let duration = (15.0 + 25.0 / SPEED * d) as i64;
            travel_times.push(duration);
            // ignore distance weighting
            distances.push(0);
        }
    }

    let matrices: Option<Vec<Matrix>> = serde_json::from_value(json!([{
        "profile": "matrix1",
        "travelTimes": travel_times,
        "distances": distances,
    }]))
    .unwrap();

    let core_problem = (problem.clone(), matrices.clone()).read_pragmatic();

    let core_problem = Arc::new(
        core_problem.unwrap_or_else(|errors| panic!("cannot read pragmatic problem: {errors}")),
    );

    let config = VrpConfigBuilder::new(core_problem.clone())
        .prebuild()
        .unwrap()
        .with_max_generations(Some(3000))
        .build()
        .unwrap_or_else(|err| panic!("cannot build default solver configuration: {err}"));
    let solution = Solver::new(core_problem.clone(), config)
        .solve()
        .unwrap_or_else(|err| panic!("cannot solver problem: {err}"));

    let solution = get_pragmatic_solution(&core_problem, &solution);

    if let Err(errs) = CheckerContext::new(core_problem, problem, matrices, solution.clone())
        .and_then(|ctx| ctx.check())
    {
        panic!(
            "unfeasible solution:\n'{}'",
            GenericError::join_many(&errs, "\n")
        );
    }

    let solution_json = serde_json::to_string_pretty(&solution).unwrap();
    // write file
    std::fs::write("solution.json", solution_json).unwrap();

    let mut completed_jobs = BTreeSet::new();
    for tour in solution.tours.iter() {
        if solution.tours.len() > 1 {
            println!("{}", tour.vehicle_id);
        }
        for stop in tour.stops.iter() {
            let index = match stop.location() {
                Some(Location::Reference { index }) => *index,
                _ => panic!("stop location is not index"),
            };
            let seconds = from_timestamp(&stop.schedule().arrival);
            let ts = format!("{:02}:{:02}", seconds / 60, seconds % 60);
            let waypoint = &locations[index];
            // assert_eq!(stop.activities().len(), 1);
            for activity in stop.activities().iter() {
                completed_jobs.insert(activity.job_id.clone());
                let desc = match &activity.job_tag {
                    Some(tag) => tag.clone(),
                    None => "".to_string(),
                };
                println!(
                    "{:06}  {:13}  {:10} {}",
                    ts, waypoint, activity.activity_type, desc
                );
            }
        }
    }
    let mut profit = 0;
    for job in jobs.iter() {
        if completed_jobs.contains(&job["id"].as_str().unwrap().to_string()) {
            profit += job["value"].as_i64().unwrap();
        }
    }
    println!("total profit: ${}", profit);
}

fn get_pragmatic_solution(problem: &CoreProblem, solution: &CoreSolution) -> Solution {
    let output_type = Default::default();
    let mut writer = std::io::BufWriter::new(Vec::new());

    write_pragmatic(problem, solution, output_type, &mut writer)
        .expect("cannot write pragmatic solution");
    let bytes = writer.into_inner().expect("cannot get bytes from writer");

    deserialize_solution(std::io::BufReader::new(bytes.as_slice()))
        .expect("cannot deserialize solution")
}
