use lazy_static::lazy_static;
use regex::Regex;

pub struct Config {
    pub job_id_filter: Regex,
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
        Config { job_id_filter }
    };
}
