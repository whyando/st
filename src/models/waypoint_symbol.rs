use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SystemSymbol(pub String);

impl<'de> Deserialize<'de> for SystemSymbol {
    fn deserialize<D>(deserializer: D) -> Result<SystemSymbol, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        // validate format
        let s = String::deserialize(deserializer)?;
        let parts: Vec<&str> = s.split('-').collect();
        if parts.len() != 2 {
            return Err(serde::de::Error::custom("Invalid system symbol"));
        }
        Ok(SystemSymbol(s))
    }
}

impl std::fmt::Display for SystemSymbol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, PartialOrd, Ord, Eq, Hash)]
pub struct WaypointSymbol(pub String);

impl WaypointSymbol {
    pub fn new(s: &str) -> WaypointSymbol {
        WaypointSymbol(s.to_string())
    }
}

impl<'de> Deserialize<'de> for WaypointSymbol {
    fn deserialize<D>(deserializer: D) -> Result<WaypointSymbol, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        // validate format
        let s = String::deserialize(deserializer)?;
        let parts: Vec<&str> = s.split('-').collect();
        if parts.len() != 3 {
            return Err(serde::de::Error::custom("Invalid waypoint symbol"));
        }
        Ok(WaypointSymbol(s))
    }
}

impl WaypointSymbol {
    pub fn system(&self) -> SystemSymbol {
        let parts: Vec<&str> = self.0.split('-').collect();
        assert_eq!(parts.len(), 3, "Invalid waypoint symbol");
        SystemSymbol(parts[0..2].join("-"))
    }
}

impl std::fmt::Display for WaypointSymbol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_waypoint_symbol_serialisation() {
        let waypoint_symbol: WaypointSymbol = serde_json::from_str("\"X1-TZ26-A1\"").unwrap();
        assert_eq!(waypoint_symbol, WaypointSymbol("X1-TZ26-A1".to_string()));
        assert_eq!(
            serde_json::to_string(&waypoint_symbol).unwrap(),
            "\"X1-TZ26-A1\""
        );
    }

    #[test]
    fn test_system_symbol_serialisation() {
        let system_symbol: SystemSymbol = serde_json::from_str("\"X1-TZ26\"").unwrap();
        assert_eq!(system_symbol, SystemSymbol("X1-TZ26".to_string()));
        assert_eq!(
            serde_json::to_string(&system_symbol).unwrap(),
            "\"X1-TZ26\""
        );
    }

    #[test]
    fn test_waypoint_symbol_system() {
        let waypoint_symbol = WaypointSymbol("X1-TZ26-A1".to_string());
        let system_symbol: SystemSymbol = waypoint_symbol.system();
        assert_eq!(system_symbol, SystemSymbol("X1-TZ26".to_string()));
    }
}
