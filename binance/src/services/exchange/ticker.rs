//! Ticker builder module for WebSocket stream management in arbitrage chains.
//!
//! This module provides a `TickerBuilder` for collecting unique symbols from triangular chains,
//! creating book ticker streams, chunking them across multiple WebSocket connections (to respect
//! limits), and spawning concurrent tasks to listen for real-time bid/ask updates. Events are
//! broadcast via a channel.

use std::collections::HashSet;

use anyhow::Context;
use engine::METRICS;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

use crate::{
    libs::binance_client::stream::{Events, StreamEvent, WebsocketStream, book_ticker_stream},
    services::{
        broadcast::TICKER_BROADCAST, exchange::chain::ChainSymbol, storage::BookTickerEvent,
    },
};

/// Builder for managing book ticker WebSocket streams across symbol chains.
#[derive(Clone)]
pub struct TickerBuilder {
    ws_streams_url: String,
    ws_max_connections: usize,
}

impl TickerBuilder {
    #[must_use]
    pub fn new(ws_streams_url: String, ws_max_connections: usize) -> Self {
        Self {
            ws_streams_url,
            ws_max_connections,
        }
    }

    /// Builds and starts book ticker streams for the given chains.
    pub async fn build_order_books(
        &self,
        token: CancellationToken,
        chains: Vec<[ChainSymbol; 3]>,
    ) -> anyhow::Result<()> {
        let symbols = Self::collect_unique_symbols(&chains);
        let streams = Self::create_streams(&symbols);

        info!(
            streams = streams.len(),
            "📡 [Network] WebSocket streams active"
        );

        let chunk_size = (streams.len() as f64 / self.ws_max_connections as f64).ceil() as usize;
        let mut tasks_set: JoinSet<anyhow::Result<()>> = JoinSet::new();

        for chunk in streams.chunks(chunk_size) {
            let ws_url = self.ws_streams_url.clone();
            let streams_chunk = chunk.to_vec();
            let token = token.clone();

            tasks_set.spawn(async move {
                Self::handle_ticker_events(ws_url, streams_chunk, token)
                    .await
                    .context("WS chunk task failed")
            });
        }

        while let Some(result) = tasks_set.join_next().await {
            match result {
                Ok(Err(e)) => {
                    error!(error = ?e, "Task failed");
                    token.cancel();
                }
                Err(e) => {
                    error!(error = ?e, "Join error");
                    token.cancel();
                }
                _ => {
                    token.cancel();
                }
            }
        }

        Ok(())
    }

    /// Handles a chunk of book ticker streams in a dedicated WebSocket connection.
    async fn handle_ticker_events(
        ws_url: String,
        streams_chunk: Vec<String>,
        token: CancellationToken,
    ) -> anyhow::Result<()> {
        let mut ws: WebsocketStream<'_, StreamEvent<_>> = WebsocketStream::new(ws_url.clone())
            .with_callback(|event: StreamEvent<Events>| {
                if let Events::BookTicker(event) = event.data {
                    let ticker = BookTickerEvent {
                        update_id: event.update_id,
                        symbol: event.symbol.clone(),
                        bid_price: event.best_bid_price,
                        bid_qty: event.best_bid_qty,
                        ask_price: event.best_ask_price,
                        ask_qty: event.best_ask_qty,
                    };

                    if let Err(e) = TICKER_BROADCAST.broadcast_event(ticker) {
                        error!(error = ?e, symbol = ?event.symbol, "Failed to broadcast ticker price");
                        return Err(anyhow::anyhow!("Failed to broadcast ticker price: {e}"));
                    }

                    METRICS.record_book_ticker_event(event.symbol.as_str());
                };

                Ok(())
            });

        ws.connect_multiple(&streams_chunk)
            .await
            .context("Failed to connect WS")?;

        ws.handle_messages(token)
            .await
            .context("Error while running WS")?;

        ws.disconnect().await;

        Ok(())
    }

    fn collect_unique_symbols(chains: &[[ChainSymbol; 3]]) -> Vec<String> {
        chains
            .iter()
            .flat_map(|chain| chain.iter())
            .map(|chain_symbol| chain_symbol.symbol.symbol.to_lowercase())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect()
    }

    fn create_streams(symbols: &[String]) -> Vec<String> {
        symbols
            .iter()
            .map(|symbol| book_ticker_stream(symbol))
            .collect()
    }
}
