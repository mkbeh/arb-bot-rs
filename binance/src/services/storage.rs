use std::collections::{HashMap, hash_map::Entry};

use rust_decimal::Decimal;

/// Changes in book ticker events (bid/ask updates for a symbol).
#[derive(Debug, Clone, Default)]
pub struct BookTickerEvent {
    /// Unique update ID for ordering events.
    pub update_id: u64,
    /// The trading symbol.
    pub symbol: String,
    /// Bid price.
    pub bid_price: Decimal,
    /// Bid quantity.
    pub bid_qty: Decimal,
    /// Ask price.
    pub ask_price: Decimal,
    /// Ask quantity.
    pub ask_qty: Decimal,
}

/// In-memory store for book ticker events, keyed by symbol.
/// Updates only if the new event has a higher update_id.
#[derive(Debug, Clone, Default)]
pub struct BookTickerStore {
    data: HashMap<String, BookTickerEvent>,
}

impl BookTickerStore {
    /// Creates a new empty store.
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
        }
    }

    /// Updates the store with the given event if it has a newer update_id.
    pub fn update(&mut self, event: BookTickerEvent) {
        match self.data.entry(event.symbol.clone()) {
            Entry::Occupied(mut entry) => {
                if event.update_id > entry.get().update_id {
                    entry.insert(event);
                }
            }
            Entry::Vacant(entry) => {
                entry.insert(event);
            }
        }
    }

    /// Retrieves the latest event for a symbol.
    pub fn get(&self, symbol: &str) -> Option<&BookTickerEvent> {
        self.data.get(symbol)
    }

    /// Returns the number of stored symbols.
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Checks if the store is empty.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}
