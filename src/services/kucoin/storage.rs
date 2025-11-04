use std::collections::HashMap;

use rust_decimal::Decimal;

#[derive(Debug, Clone, Default)]
pub struct BookTickerEvent {
    pub sequence_id: u64,
    pub symbol: String,
    pub price: Decimal,
    pub qty: Decimal,
    pub order_side: OrderSide,
}

#[derive(Debug, Clone, Default)]
pub enum OrderSide {
    #[default]
    Bid,
    Asc,
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
                if event.sequence_id > entry.get().sequence_id {
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

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}
