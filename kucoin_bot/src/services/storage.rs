use std::collections::{HashMap, hash_map::Entry};

use rust_decimal::Decimal;

/// Changes in book ticker events (bid/ask updates for a symbol).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BookTickerEventChanges {
    // The symbol affected by the changes.
    pub symbol: String,
    /// Optional bid update event.
    pub bid: Option<BookTickerEvent>,
    /// Optional ask update event.
    pub ask: Option<BookTickerEvent>,
}

/// A single book ticker event (price/quantity update).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BookTickerEvent {
    /// Sequence ID for ordering events.
    pub sequence_id: u64,
    /// The trading symbol.
    pub symbol: String,
    /// Updated price.
    pub price: Decimal,
    /// Updated quantity.
    pub qty: Decimal,
}

/// In-memory store for book ticker events, keyed by symbol.
/// Updates only if sequence_id is newer and values are non-zero.
#[derive(Debug, Clone, Default)]
pub struct BookTickerStore {
    data: HashMap<String, BookTickerEvent>,
}

impl BookTickerEventChanges {
    /// Creates a new changes instance for the given symbol.
    #[must_use]
    pub fn new(symbol: &str) -> Self {
        Self {
            symbol: symbol.to_owned(),
            ..Default::default()
        }
    }
}

impl BookTickerStore {
    /// Creates a new empty store.
    #[must_use]
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
        }
    }

    /// Updates the store with an optional event if valid (non-zero price/qty and newer sequence).
    /// Returns true if updated or no event (always succeeds unless invalid).
    pub fn update_if_valid(&mut self, event: Option<BookTickerEvent>) -> bool {
        if let Some(event) = event {
            if event.price.is_zero() || event.qty.is_zero() {
                return false;
            }
            self.update(event);
        }
        true
    }

    /// Retrieves the latest event for a symbol.
    #[must_use]
    pub fn get(&self, symbol: &str) -> Option<&BookTickerEvent> {
        self.data.get(symbol)
    }

    /// Returns the number of stored symbols.
    #[must_use]
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Checks if the store is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Internal update: inserts or replaces if sequence_id is newer.
    fn update(&mut self, event: BookTickerEvent) {
        match self.data.entry(event.symbol.clone()) {
            Entry::Occupied(mut entry) => {
                if event.sequence_id > entry.get().sequence_id {
                    entry.insert(event);
                }
            }
            Entry::Vacant(entry) => {
                entry.insert(event);
            }
        }
    }
}
