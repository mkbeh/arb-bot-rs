//! Ticker builder module for WebSocket stream management in arbitrage chains.
//!
//! This module provides a `TickerBuilder` for collecting unique symbols from triangular chains,
//! creating book ticker streams, chunking them across multiple WebSocket connections (to respect
//! limits), and spawning concurrent tasks to listen for real-time bid/ask updates. Events are
//! broadcast via a channel.

use std::collections::HashSet;

use anyhow::bail;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;
use tracing::error;

use crate::{
    libs::kucoin_api::{
        BaseInfo,
        stream::{
            Events, Level2Update, MessageEvents, OrderRow, Topic, WebsocketStream,
            order_book_increment_topic,
        },
    },
    services::{
        kucoin::{
            broadcast::TICKER_BROADCAST,
            exchange::chain::ChainSymbol,
            storage::{BookTickerEvent, BookTickerEventChanges},
        },
        metrics::METRICS,
    },
};

/// Builder for managing book ticker WebSocket streams across symbol chains.
#[derive(Clone)]
pub struct TickerBuilder {
    base_info_api: BaseInfo,
    ws_symbols_limit: usize,
}

impl TickerBuilder {
    pub fn new(base_info_api: BaseInfo) -> TickerBuilder {
        Self {
            base_info_api,
            ws_symbols_limit: 100,
        }
    }

    /// Builds and starts book ticker streams for the given chains.
    pub async fn build_order_books(
        &self,
        token: CancellationToken,
        chains: Vec<[ChainSymbol; 3]>,
    ) -> anyhow::Result<()> {
        let (api_token, ws_endpoint, ping_interval) =
            match self.base_info_api.get_bullet_public().await {
                Ok(resp) => (
                    resp.data.token,
                    resp.data.instance_servers[0].endpoint.clone(),
                    resp.data.instance_servers[0].ping_interval,
                ),
                Err(err) => bail!("Error getting bullet public: {}", err),
            };

        let unique_symbols: Vec<&str> = chains
            .iter()
            .flat_map(|chain| chain.iter())
            .map(|chain_symbol| chain_symbol.symbol.symbol.as_str())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();

        let mut tasks_set: JoinSet<anyhow::Result<()>> = JoinSet::new();
        for chunk in unique_symbols.chunks(self.ws_symbols_limit) {
            let ws_endpoint = ws_endpoint.clone();
            let topics = [order_book_increment_topic(chunk)];
            let api_token = api_token.clone();
            let token = token.clone();

            tasks_set.spawn(Self::handle_events_task(
                ws_endpoint,
                topics,
                api_token,
                token,
                ping_interval,
            ));
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
    async fn handle_events_task(
        ws_endpoint: String,
        topics: [Topic; 1],
        api_token: String,
        token: CancellationToken,
        ping_interval: u64,
    ) -> anyhow::Result<()> {
        let mut ws = WebsocketStream::<'_, Events>::new(ws_endpoint.clone(), ping_interval)
            .with_callback(Self::handle_events_callback());

        ws.connect(&topics, api_token).await.map_err(|e| {
            error!(error = ?e, ws_url = %ws_endpoint, "Failed to connect websocket");
            e
        })?;

        if let Err(e) = ws.handle_messages(token).await {
            error!(error = ?e, ws_url = %ws_endpoint, "Error while running websocket");
            return Err(e);
        }

        ws.disconnect().await;
        Ok(())
    }

    fn handle_events_callback() -> impl Fn(Events) -> anyhow::Result<()> + Send + Sync + 'static {
        move |event: Events| {
            if let Events::Message(event) = event
                && let MessageEvents::IncrementOrderBook(message) = *event
            {
                Self::process_order_book_update(&message)?;
            }
            Ok(())
        }
    }

    fn process_order_book_update(update: &Level2Update) -> anyhow::Result<()> {
        let create_ticker_event = |symbol: &str, row: OrderRow| -> BookTickerEvent {
            let OrderRow(price, qty, sequence_id) = row;
            BookTickerEvent {
                symbol: symbol.to_string(),
                sequence_id,
                price,
                qty,
            }
        };

        let symbol = &update.symbol;
        let mut changes = BookTickerEventChanges::new(symbol);

        if let Some(bid_row) = update.latest_bid() {
            changes.bid = Some(create_ticker_event(symbol, bid_row));
        }
        if let Some(ask_row) = update.latest_ask() {
            changes.ask = Some(create_ticker_event(symbol, ask_row));
        }

        if changes != BookTickerEventChanges::default() {
            if let Err(e) = TICKER_BROADCAST.broadcast_event(changes) {
                error!(error = ?e, symbol = %symbol, "Failed to broadcast changes event");
                // Don't bail here to keep WS alive; just log and continue
            }
            METRICS.add_book_ticker_event(symbol);
        }

        Ok(())
    }
}
