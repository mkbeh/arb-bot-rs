use std::sync::{Arc, LazyLock};

use dashmap::DashMap;
use tokio::sync::broadcast;

use crate::services::binance::storage::BookTickerEvent;

pub static TICKER_BROADCAST: LazyLock<TickerBroadcast> = LazyLock::new(TickerBroadcast::new);


pub struct TickerBroadcast {
    channels: Arc<DashMap<String, broadcast::Sender<BookTickerEvent>>>,
}

impl TickerBroadcast {
    fn new() -> Self {
        Self {
            channels: Arc::new(DashMap::new()),
        }
    }

    pub fn get_channel(&self, symbol: &str) -> broadcast::Sender<BookTickerEvent> {
        self.channels
            .entry(symbol.to_string())
            .or_insert_with(|| {
                let (tx, _rx) = broadcast::channel(1);
                tx
            })
            .clone()
    }

    pub fn broadcast_event(&self, event: BookTickerEvent) -> Result<(), String> {
        let tx = self.get_channel(&event.symbol);
        if let Err(e) = tx.send(event) {
            return Err(format!("Failed to broadcast: {e}"));
        };
        Ok(())
    }

    pub fn subscribe(&self, ticker: &str) -> broadcast::Receiver<BookTickerEvent> {
        let tx = self.get_channel(ticker);
        tx.subscribe()
    }
}
