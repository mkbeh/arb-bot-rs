use std::sync::Arc;

use arc_swap::ArcSwap;
use dashmap::DashMap;
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

#[derive(Debug, Clone)]
pub struct BookTickerStore {
    data: DashMap<String, Arc<ArcSwap<BookTickerEvent>>>,
}

impl BookTickerStore {
    pub fn new() -> Self {
        Self {
            data: DashMap::new(),
        }
    }

    /// Update data for a symbol (atomic replacement).
    pub fn update(&self, event: BookTickerEvent) {
        let symbol = event.symbol.clone();
        let entry = self
            .data
            .entry(symbol)
            .or_insert_with(|| Arc::new(ArcSwap::from(Arc::new(BookTickerEvent::default()))));

        entry.value().store(Arc::new(event));
    }

    /// Read the last data for a symbol.
    pub fn get(&self, symbol: &str) -> Option<Arc<BookTickerEvent>> {
        self.data.get(symbol).map(|entry| entry.value().load_full())
    }
}
