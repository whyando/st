/// Track the allocations of current credits of the agent
use log::*;
use std::collections::BTreeMap;
use std::sync::Mutex;

#[derive(Debug)]
pub struct Ledger {
    total_credits: Mutex<i64>,
    reserved_credits: Mutex<BTreeMap<String, i64>>,
}

impl Ledger {
    pub fn new(start_credits: i64) -> Self {
        Ledger {
            total_credits: Mutex::new(start_credits),
            reserved_credits: Mutex::new(BTreeMap::new()),
        }
    }

    pub fn set_credits(&self, credits: i64) {
        *self.total_credits.lock().unwrap() = credits;
    }

    pub fn credits(&self) -> i64 {
        *self.total_credits.lock().unwrap()
    }

    pub fn reserve_credits(&self, ship_symbol: &str, amount: i64) {
        debug!("Setting {} credits reserved for {}", amount, ship_symbol);
        let mut reserved_credits = self.reserved_credits.lock().unwrap();
        reserved_credits.insert(ship_symbol.to_string(), amount);
    }

    pub fn reserve_credits_delta(&self, ship_symbol: &str, delta: i64) {
        debug!(
            "Adjusting reserved credits for {} by {}",
            ship_symbol, delta
        );
        let mut reserved_credits = self.reserved_credits.lock().unwrap();
        reserved_credits
            .entry(ship_symbol.to_string())
            .and_modify(|e| *e += delta);
    }

    pub fn available_credits(&self) -> i64 {
        self.credits() - self.reserved_credits()
    }

    pub fn reserved_credits(&self) -> i64 {
        let reserved_credits = self.reserved_credits.lock().unwrap();
        reserved_credits.values().sum::<i64>()
    }
}
