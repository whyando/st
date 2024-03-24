use lazy_static::lazy_static;
use regex::Regex;

pub struct Config {
    pub job_id_filter: Regex,
    pub override_construction_supply_check: bool,
    pub scrap_all_ships: bool,
}

lazy_static! {
    pub static ref CONFIG: Config = {
        let job_id_filter = match std::env::var("JOB_ID_FILTER") {
            Ok(val) if val.is_empty() => None,
            Ok(val) => Some(val),
            Err(_) => None,
        };
        let job_id_filter = match job_id_filter {
            Some(val) => Regex::new(&val).expect("Invalid JOB_ID_FILTER regex"),
            None => Regex::new(".*").expect("Invalid default regex"),
        };
        let override_construction_supply_check =
            std::env::var("OVERRIDE_CONSTRUCTION_SUPPLY_CHECK")
                .map(|val| val == "1")
                .unwrap_or(false);
        let scrap_all_ships = std::env::var("SCRAP_ALL_SHIPS")
            .map(|val| val == "1")
            .unwrap_or(false);
        Config {
            job_id_filter,
            override_construction_supply_check,
            scrap_all_ships,
        }
    };
}
