use std::fmt::{self, Display, Formatter};

use super::{
    ShipEngine, ShipFrame, ShipModule, ShipMount, ShipReactor, SymbolNameDescr, WaypointSymbol,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Market {
    pub symbol: WaypointSymbol,
    pub transactions: Vec<MarketTransaction>,
    pub imports: Vec<SymbolNameDescr>,
    pub exports: Vec<SymbolNameDescr>,
    pub exchange: Vec<SymbolNameDescr>,
    pub trade_goods: Vec<MarketTradeGood>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketRemoteView {
    // no transactions or trade goods
    pub symbol: WaypointSymbol,
    pub imports: Vec<SymbolNameDescr>,
    pub exports: Vec<SymbolNameDescr>,
    pub exchange: Vec<SymbolNameDescr>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketTradeGood {
    pub symbol: String,
    pub trade_volume: i64,
    pub _type: MarketType,
    pub supply: MarketSupply,
    pub activity: Option<MarketActivity>,
    pub purchase_price: i64,
    pub sell_price: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MarketType {
    #[serde(rename = "IMPORT")]
    Import,
    #[serde(rename = "EXPORT")]
    Export,
    #[serde(rename = "EXCHANGE")]
    Exchange,
}

impl Display for MarketType {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{}", format!("{:?}", self).to_uppercase())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum MarketSupply {
    #[serde(rename = "SCARCE")]
    Scarce,
    #[serde(rename = "LIMITED")]
    Limited,
    #[serde(rename = "MODERATE")]
    Moderate,
    #[serde(rename = "HIGH")]
    High,
    #[serde(rename = "ABUNDANT")]
    Abundant,
}

impl Display for MarketSupply {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{}", format!("{:?}", self).to_uppercase())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MarketActivity {
    #[serde(rename = "WEAK")]
    Weak,
    #[serde(rename = "GROWING")]
    Growing,
    #[serde(rename = "STRONG")]
    Strong,
    #[serde(rename = "RESTRICTED")]
    Restricted,
}

impl Display for MarketActivity {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{}", format!("{:?}", self).to_uppercase())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Shipyard {
    pub symbol: WaypointSymbol,
    pub ship_types: Vec<ShipType>,
    pub modifications_fee: i64,
    // pub transactions: Vec<_>,
    pub ships: Vec<ShipyardShip>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShipyardShip {
    #[serde(rename = "type")]
    pub ship_type: String,
    pub name: String,
    pub description: String,
    pub supply: String,
    pub purchase_price: i64,
    pub frame: ShipFrame,
    pub reactor: ShipReactor,
    pub engine: ShipEngine,
    pub modules: Vec<ShipModule>,
    pub mounts: Vec<ShipMount>,
    // pub crew: ShipCrew,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShipyardRemoteView {
    pub symbol: WaypointSymbol,
    pub ship_types: Vec<ShipType>,
    pub modifications_fee: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShipType {
    #[serde(rename = "type")]
    pub ship_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketTransaction {
    pub waypoint_symbol: WaypointSymbol,
    pub ship_symbol: String,
    pub trade_symbol: String,
    #[serde(rename = "type")]
    pub _type: String,
    pub units: i64,
    pub price_per_unit: i64,
    pub total_price: i64,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScrapTransaction {
    pub waypoint_symbol: WaypointSymbol,
    pub ship_symbol: String,
    pub total_price: i64,
    pub timestamp: DateTime<Utc>,
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn test_market_good() {
        let good1_json = r#"{
            "symbol": "FOOD",
            "tradeVolume": 60,
            "type": "IMPORT",
            "supply": "SCARCE",
            "activity": "WEAK",
            "purchasePrice": 4702,
            "sellPrice": 2332
        }"#;
        let good2_json = r#"{
            "symbol": "FUEL",
            "tradeVolume": 180,
            "type": "EXCHANGE",
            "supply": "MODERATE",
            "purchasePrice": 72,
            "sellPrice": 68
        }"#;
        let good1: MarketTradeGood = serde_json::from_str(good1_json).unwrap();
        let good2: MarketTradeGood = serde_json::from_str(good2_json).unwrap();
        assert_eq!(good1.symbol, "FOOD");
        assert_eq!(good2.symbol, "FUEL");
    }

    #[test]
    fn test_supply_order() {
        use MarketSupply::*;
        assert!(Scarce < Limited);
        assert!(Moderate < High);
        assert!(High < Abundant);
        assert!(Moderate >= Moderate);
        assert!(High >= Moderate);
    }

    #[test]
    fn test_market_transaction() {
        let json = r#"{
            "waypointSymbol": "X1-HB61-A1",
            "shipSymbol": "100L-TRADER2-1",
            "tradeSymbol": "EQUIPMENT",
            "type": "SELL",
            "units": 20,
            "pricePerUnit": 3486,
            "totalPrice": 69720,
            "timestamp": "2024-02-05T01:10:41.237Z"
          }"#;
        let transaction: MarketTransaction = serde_json::from_str(json).unwrap();
        assert_eq!(
            transaction.waypoint_symbol,
            WaypointSymbol("X1-HB61-A1".to_string())
        );
    }

    #[test]
    fn test_enum_to_string() {
        let supply = MarketSupply::Scarce;
        assert_eq!(format!("{}", supply), "SCARCE");
        assert_eq!(supply.to_string(), "SCARCE");
    }
}
