use std::sync::{Arc, LazyLock};

use dashmap::DashMap;
use tokio::sync::watch;

use crate::services::kucoin::storage::BookTickerEventChanges;

/// Global broadcast system for book ticker events using watch channels per symbol.
pub static TICKER_BROADCAST: LazyLock<TickerBroadcast> = LazyLock::new(TickerBroadcast::new);

/// Manages per-symbol watch channels for broadcasting ticker changes.
pub struct TickerBroadcast {
    channels: Arc<DashMap<String, watch::Sender<BookTickerEventChanges>>>,
}

impl TickerBroadcast {
    /// Creates a new broadcast instance with an empty channel map.
    fn new() -> Self {
        Self {
            channels: Arc::new(DashMap::new()),
        }
    }

    /// Retrieves or creates a sender for the given symbol (clones if exists).
    pub fn get_sender(&self, symbol: &str) -> watch::Sender<BookTickerEventChanges> {
        self.channels
            .entry(symbol.to_string())
            .or_insert_with(|| {
                let (tx, _rx) = watch::channel(BookTickerEventChanges::default());
                tx
            })
            .clone()
    }

    /// Broadcasts an event to the symbol's channel.
    pub fn broadcast_event(&self, event: BookTickerEventChanges) -> Result<(), String> {
        let tx = self.get_sender(&event.symbol);
        tx.send(event)
            .map_err(|e| format!("Failed to broadcast: {e}"))
    }

    /// Subscribes to changes for the given symbol (creates channel if missing).
    pub fn subscribe(&self, ticker: &str) -> watch::Receiver<BookTickerEventChanges> {
        let tx = self.get_sender(ticker);
        tx.subscribe()
    }
}
