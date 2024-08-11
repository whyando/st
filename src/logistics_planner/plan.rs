use super::*;
use chrono::DateTime;
use chrono::Utc;
use std::collections::BTreeMap;
use std::sync::Arc;
use vrp_pragmatic::core::models::{Problem as CoreProblem, Solution as CoreSolution};
use vrp_pragmatic::core::solver::Solver;
use vrp_pragmatic::core::solver::VrpConfigBuilder;
use vrp_pragmatic::format::problem::*;
use vrp_pragmatic::format::solution::*;
use vrp_pragmatic::format::Location;

fn location_index(locations: &mut Vec<WaypointSymbol>, location: &WaypointSymbol) -> usize {
    match locations.iter().position(|x| x == location) {
        Some(index) => index,
        None => {
            let index = locations.len();
            locations.push(location.clone());
            index
        }
    }
}

// return timestamp in RFC3339 format of seconds since 0000-01-01T00:00:00Z
fn get_timestamp(seconds: i64) -> String {
    let dt: DateTime<Utc> = DateTime::from_timestamp(seconds, 0).unwrap();
    dt.to_rfc3339()
}

fn from_timestamp(timestamp: &str) -> i64 {
    DateTime::parse_from_rfc3339(timestamp).unwrap().timestamp()
}

pub fn run_planner(
    ships: &[LogisticShip],
    tasks: &[Task],
    duration_matrix: &BTreeMap<WaypointSymbol, BTreeMap<WaypointSymbol, i64>>,
    constraints: &PlannerConstraints,
) -> (BTreeMap<Task, Option<String>>, Vec<ShipSchedule>) {
    // start by defining vrp problem
    // docs: https://reinterpretcat.github.io/vrp/concepts/pragmatic/index.html

    let mut task_job_id_map: BTreeMap<String, &Task> = BTreeMap::new();

    let mut locations = vec![];
    let time_window = vec![vec![
        get_timestamp(0),
        get_timestamp(constraints.plan_length.num_seconds()),
    ]];

    let jobs: Vec<Job> = tasks
        .iter()
        .map(|task| {
            match &task.actions {
                TaskActions::VisitLocation { waypoint, action } => {
                    let id = format!("Visit-{}-{:?}", waypoint, action);
                    let tag = format!("[{}] {:?}", waypoint, action);
                    task_job_id_map.insert(id.clone(), task);
                    Job {
                        id,
                        pickups: None,
                        deliveries: None,
                        replacements: None,
                        services: Some(vec![JobTask {
                            places: vec![JobPlace {
                                location: Location::Reference {
                                    index: location_index(&mut locations, waypoint),
                                },
                                duration: 0.0,
                                times: Some(time_window.clone()),
                                tag: Some(tag),
                            }],
                            demand: None, // service: no demand
                            order: None,
                        }]),
                        skills: None,
                        value: Some(task.value as f64),
                        group: None,
                        compatibility: None,
                    }
                }
                TaskActions::TransportCargo {
                    src,
                    dest,
                    src_action,
                    dest_action,
                } => {
                    let (good, units) = match src_action {
                        Action::BuyGoods(good, units) => (good, units),
                        _ => panic!("unexpected source action"),
                    };
                    let (dest_good, dest_units) = match dest_action {
                        Action::SellGoods(good, units) => (good, units),
                        Action::DeliverConstruction(good, units) => (good, units),
                        Action::DeliverContract(good, units) => (good, units),
                        _ => panic!("unexpected destination action"),
                    };
                    assert_eq!(good, dest_good);
                    assert_eq!(units, dest_units);
                    let id = format!("Transport-{}", good);
                    task_job_id_map.insert(id.clone(), task);
                    Job {
                        id,
                        pickups: Some(vec![JobTask {
                            places: vec![JobPlace {
                                location: Location::Reference {
                                    index: location_index(&mut locations, src),
                                },
                                duration: 0.0,
                                times: Some(time_window.clone()),
                                tag: Some(format!("[{}] {:?} {} {}", src, src_action, units, good)),
                            }],
                            demand: Some(vec![*units as i32]),
                            order: None,
                        }]),
                        deliveries: Some(vec![JobTask {
                            places: vec![JobPlace {
                                location: Location::Reference {
                                    index: location_index(&mut locations, dest),
                                },
                                duration: 0.0,
                                times: Some(time_window.clone()),
                                tag: Some(format!(
                                    "[{}] {:?} {} {}",
                                    dest, dest_action, units, good
                                )),
                            }],
                            demand: Some(vec![*units as i32]),
                            order: None,
                        }]),
                        replacements: None,
                        services: None,
                        skills: None,
                        value: Some(task.value as f64), // usually profit
                        group: None,
                        compatibility: None,
                    }
                }
            }
        })
        .collect();

    let vehicles: Vec<VehicleType> = ships
        .iter()
        .map(|ship| {
            VehicleType {
                type_id: ship.symbol.clone(),
                vehicle_ids: vec![ship.symbol.clone()],
                profile: VehicleProfile {
                    matrix: "cruise".to_string(),
                    scale: None, // default 1
                },
                costs: VehicleCosts {
                    fixed: None,
                    distance: 0.0001,
                    time: 0.0001,
                },
                shifts: vec![VehicleShift {
                    start: ShiftStart {
                        earliest: get_timestamp(0),
                        latest: None,
                        location: Location::Reference {
                            index: location_index(&mut locations, &ship.start_waypoint),
                        },
                    },
                    end: None,
                    breaks: None,
                    reloads: None,
                    recharges: None,
                }],
                capacity: vec![ship.capacity as i32],
                skills: None,
                limits: None,
            }
        })
        .collect();

    let problem = Problem {
        plan: Plan {
            jobs,
            relations: None,
            clustering: None,
        },
        fleet: Fleet {
            vehicles,
            profiles: vec![MatrixProfile {
                name: "cruise".to_string(),
                speed: None,
            }],
            resources: None,
        },
        objectives: None,
    };

    // cruise matrix
    let matrix = {
        let mut travel_times = vec![];
        let mut distances = vec![];
        for src in &locations {
            for dest in &locations {
                let duration = duration_matrix.get(src).unwrap().get(dest).unwrap();
                travel_times.push(*duration);
                distances.push(0); // 0 weight on distance
            }
        }
        Matrix {
            profile: Some("cruise".to_string()),
            timestamp: None,
            travel_times,
            distances,
            error_codes: None,
        }
    };
    let matrices = Some(vec![matrix]);

    let core_problem = (problem.clone(), matrices.clone()).read_pragmatic();
    let core_problem = Arc::new(
        core_problem.unwrap_or_else(|errors| panic!("cannot read pragmatic problem: {errors}")),
    );

    let config = VrpConfigBuilder::new(core_problem.clone())
        .prebuild()
        .unwrap()
        .with_max_generations(Some(3000))
        .with_max_time(Some(constraints.max_compute_time.num_seconds() as usize))
        .build()
        .unwrap_or_else(|err| panic!("cannot build default solver configuration: {err}"));
    let solution = Solver::new(core_problem.clone(), config)
        .solve()
        .unwrap_or_else(|err| panic!("cannot solver problem: {err}"));
    let solution = get_pragmatic_solution(&core_problem, &solution);
    // log::info!("solution: {:#?}", solution);

    // write file
    //let solution_json = serde_json::to_string_pretty(&solution).unwrap();
    //std::fs::create_dir_all("./output").unwrap();
    //std::fs::write("./output/logistics_plan_pragmatic.json", solution_json).unwrap();

    // solution checker is bugged?
    // if let Err(errs) = CheckerContext::new(core_problem, problem, matrices, solution.clone())
    //     .and_then(|ctx| ctx.check())
    // {
    //     panic!(
    //         "unfeasible solution:\n'{}'",
    //         GenericError::join_many(&errs, "\n")
    //     );
    // }

    let mut task_result = BTreeMap::<Task, Option<String>>::new();
    let ship_schedules = ships
        .iter()
        .map(|ship| {
            let vehicle = solution
                .tours
                .iter()
                .find(|tour| tour.vehicle_id == ship.symbol);
            let vehicle = match vehicle {
                Some(vehicle) => vehicle,
                None => {
                    return ShipSchedule {
                        ship: ship.clone(),
                        actions: vec![],
                    };
                }
            };
            let mut actions = vec![];
            for stop in vehicle.stops.iter() {
                let arrival = from_timestamp(&stop.schedule().arrival);
                let location_index: usize = match stop.location() {
                    Some(Location::Reference { index }) => *index,
                    _ => panic!("unexpected location type"),
                };
                let waypoint_symbol = locations[location_index].clone();
                // let departure = from_timestamp(&stop.schedule().departure);
                for activity in stop.activities().iter() {
                    if activity.job_id == "departure" || activity.job_id == "arrival" {
                        // special job ids
                        continue;
                    }
                    let task = *task_job_id_map
                        .get(&activity.job_id)
                        .expect("cannot find task for job id");
                    task_result.insert(task.clone(), Some(ship.symbol.clone()));
                    let sa = task_to_scheduled_action(
                        task,
                        activity.activity_type.as_str(),
                        Some(arrival),
                    );
                    assert_eq!(sa.waypoint, waypoint_symbol);
                    actions.push(sa);
                }
            }
            ShipSchedule {
                ship: ship.clone(),
                actions,
            }
        })
        .collect();
    if let Some(unassigned_jobs) = &solution.unassigned {
        for unassigned_job in unassigned_jobs {
            let task = *task_job_id_map
                .get(&unassigned_job.job_id)
                .expect("cannot find task for job id");
            task_result.insert(task.clone(), None);
        }
    }
    // make sure all tasks are accounted for
    for task in tasks {
        assert!(task_result.contains_key(task));
    }
    (task_result, ship_schedules)
}

pub fn task_to_scheduled_action(
    task: &Task,
    activity_type: &str,
    arrival: Option<i64>,
) -> ScheduledAction {
    let (waypoint, action, task_completed) = match &task.actions {
        TaskActions::VisitLocation { waypoint, action } => (waypoint, action, Some(task.clone())),
        TaskActions::TransportCargo {
            src,
            dest,
            src_action,
            dest_action,
        } => match activity_type {
            "pickup" => (src, src_action, None),
            "delivery" => (dest, dest_action, Some(task.clone())),
            _ => panic!("unexpected activity type"),
        },
    };
    ScheduledAction {
        waypoint: waypoint.clone(),
        action: action.clone(),
        timestamp: arrival.unwrap_or_default(),
        task_completed,
    }
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

#[cfg(test)]
mod test {
    use super::*;
    use chrono::Duration;

    #[test]
    fn test_run_planner() {
        pretty_env_logger::formatted_timed_builder()
            .is_test(true)
            .filter_level(log::LevelFilter::Debug)
            .try_init()
            .ok();
        let ships = vec![
            LogisticShip {
                symbol: "SHIP1".to_string(),
                capacity: 100,
                speed: 10,
                start_waypoint: WaypointSymbol::new("X1-S1-W1"),
            },
            LogisticShip {
                symbol: "SHIP2".to_string(),
                capacity: 200,
                speed: 20,
                start_waypoint: WaypointSymbol::new("X1-S1-W1"),
            },
        ];
        let tasks = vec![
            Task {
                id: "TASK1".to_string(),
                actions: TaskActions::VisitLocation {
                    waypoint: WaypointSymbol::new("X1-S1-W1"),
                    action: Action::RefreshMarket,
                },
                value: 1000,
            },
            Task {
                id: "TASK2".to_string(),
                actions: TaskActions::VisitLocation {
                    waypoint: WaypointSymbol::new("X1-S1-W2"),
                    action: Action::RefreshShipyard,
                },
                value: 1000,
            },
            Task {
                id: "TASK3".to_string(),
                actions: TaskActions::TransportCargo {
                    src: WaypointSymbol::new("X1-S1-W1"),
                    dest: WaypointSymbol::new("X1-S1-W2"),
                    src_action: Action::BuyGoods("FOOD".to_string(), 10),
                    dest_action: Action::SellGoods("FOOD".to_string(), 10),
                },
                value: 5000,
            },
        ];
        let constraints = PlannerConstraints {
            plan_length: Duration::try_hours(24).unwrap(),
            max_compute_time: Duration::try_seconds(1).unwrap(),
        };
        let matrix = {
            let mut duration_matrix: BTreeMap<WaypointSymbol, BTreeMap<WaypointSymbol, i64>> =
                BTreeMap::new();
            duration_matrix.insert(WaypointSymbol::new("X1-S1-W1"), {
                let mut dests = BTreeMap::new();
                dests.insert(WaypointSymbol::new("X1-S1-W1"), 0);
                dests.insert(WaypointSymbol::new("X1-S1-W2"), 100);
                dests
            });
            duration_matrix.insert(WaypointSymbol::new("X1-S1-W2"), {
                let mut dests = BTreeMap::new();
                dests.insert(WaypointSymbol::new("X1-S1-W1"), 100);
                dests.insert(WaypointSymbol::new("X1-S1-W2"), 0);
                dests
            });
            duration_matrix
        };
        let (assignments, schedule) = run_planner(&ships, &tasks, &matrix, &constraints);
        assert_eq!(schedule.len(), 2);
        assert_eq!(assignments.len(), 3);
    }
}
