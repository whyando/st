use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Ord, Eq, Hash, PartialOrd)]
pub struct SystemSymbol(String);

impl SystemSymbol {
    pub fn new(s: &str) -> SystemSymbol {
        SystemSymbol::parse(s).unwrap()
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn parse(s: &str) -> Result<SystemSymbol, String> {
        let parts: Vec<&str> = s.split('-').collect();
        if parts.len() != 2 {
            return Err("Invalid system symbol".to_string());
        }
        Ok(SystemSymbol(s.to_string()))
    }
}

impl<'de> Deserialize<'de> for SystemSymbol {
    fn deserialize<D>(deserializer: D) -> Result<SystemSymbol, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        SystemSymbol::parse(&s).map_err(serde::de::Error::custom)
    }
}

impl std::fmt::Display for SystemSymbol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, PartialOrd, Ord, Eq, Hash)]
pub struct WaypointSymbol(String);

impl WaypointSymbol {
    pub fn new(s: &str) -> WaypointSymbol {
        WaypointSymbol::parse(s).unwrap()
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn parse(s: &str) -> Result<WaypointSymbol, String> {
        let parts: Vec<&str> = s.split('-').collect();
        if parts.len() != 3 {
            return Err("Invalid waypoint symbol".to_string());
        }
        Ok(WaypointSymbol(s.to_string()))
    }
}

impl<'de> Deserialize<'de> for WaypointSymbol {
    fn deserialize<D>(deserializer: D) -> Result<WaypointSymbol, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        WaypointSymbol::parse(&s).map_err(serde::de::Error::custom)
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
        assert_eq!(waypoint_symbol, WaypointSymbol::new("X1-TZ26-A1"));
        assert_eq!(
            serde_json::to_string(&waypoint_symbol).unwrap(),
            "\"X1-TZ26-A1\""
        );
    }

    #[test]
    fn test_system_symbol_serialisation() {
        let system_symbol: SystemSymbol = serde_json::from_str("\"X1-TZ26\"").unwrap();
        assert_eq!(system_symbol, SystemSymbol::new("X1-TZ26"));
        assert_eq!(
            serde_json::to_string(&system_symbol).unwrap(),
            "\"X1-TZ26\""
        );
    }

    #[test]
    fn test_waypoint_symbol_system() {
        let waypoint_symbol = WaypointSymbol::new("X1-TZ26-A1");
        let system_symbol: SystemSymbol = waypoint_symbol.system();
        assert_eq!(system_symbol, SystemSymbol::new("X1-TZ26"));
    }
}
