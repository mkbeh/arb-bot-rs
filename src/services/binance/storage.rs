use std::collections::HashMap;

use rust_decimal::Decimal;

#[derive(Debug, Clone, Default)]
pub struct BookTickerEvent {
    pub update_id: u64,
    pub symbol: String,
    pub best_bid_price: Decimal,
    pub best_bid_qty: Decimal,
    pub best_ask_price: Decimal,
    pub best_ask_qty: Decimal,
}

#[derive(Debug, Clone, Default)]
pub struct BookTickerStore {
    data: HashMap<String, BookTickerEvent>,
}

impl BookTickerStore {
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
        }
    }

    pub fn update(&mut self, event: BookTickerEvent) {
        match self.data.entry(event.symbol.clone()) {
            std::collections::hash_map::Entry::Occupied(mut entry) => {
                if event.update_id > entry.get().update_id {
                    entry.insert(event);
                }
            }
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(event);
            }
        }
    }

    pub fn get(&self, symbol: &str) -> Option<&BookTickerEvent> {
        self.data.get(symbol)
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }
}
