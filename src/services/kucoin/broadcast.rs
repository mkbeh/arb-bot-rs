use std::sync::{Arc, LazyLock};

use dashmap::DashMap;
use tokio::sync::watch;

use crate::services::kucoin::storage::BookTickerEventChanges;

pub static TICKER_BROADCAST: LazyLock<TickerBroadcast> = LazyLock::new(TickerBroadcast::new);

pub struct TickerBroadcast {
    channels: Arc<DashMap<String, watch::Sender<BookTickerEventChanges>>>,
}

impl TickerBroadcast {
    fn new() -> Self {
        Self {
            channels: Arc::new(DashMap::new()),
        }
    }

    pub fn get_channel(&self, symbol: &str) -> watch::Sender<BookTickerEventChanges> {
        self.channels
            .entry(symbol.to_string())
            .or_insert_with(|| {
                let (tx, _rx) = watch::channel(BookTickerEventChanges::default());
                tx
            })
            .clone()
    }

    pub fn broadcast_event(&self, event: BookTickerEventChanges) -> Result<(), String> {
        let tx = self.get_channel(&event.symbol);
        tx.send(event)
            .map_err(|e| format!("Failed to broadcast: {e}"))
    }

    pub fn subscribe(&self, ticker: &str) -> watch::Receiver<BookTickerEventChanges> {
        let tx = self.get_channel(ticker);
        tx.subscribe()
    }
}
