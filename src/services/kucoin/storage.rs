use std::collections::HashMap;

use rust_decimal::Decimal;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct BookTickerEventChanges {
    pub symbol: String,
    pub bid: Option<BookTickerEvent>,
    pub ask: Option<BookTickerEvent>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct BookTickerEvent {
    pub sequence_id: u64,
    pub symbol: String,
    pub price: Decimal,
    pub qty: Decimal,
}

#[derive(Debug, Clone, Default)]
pub struct BookTickerStore {
    data: HashMap<String, BookTickerEvent>,
}

impl BookTickerEventChanges {
    pub fn new(symbol: &str) -> Self {
        Self {
            symbol: symbol.to_string(),
            ..Default::default()
        }
    }
}

impl BookTickerStore {
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
        }
    }

    pub fn update_if_valid(&mut self, event: Option<BookTickerEvent>) -> bool {
        if let Some(event) = event {
            if event.price.is_zero() || event.qty.is_zero() {
                return false;
            }
            self.update(event);
        }
        true
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

    fn update(&mut self, event: BookTickerEvent) {
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
}
