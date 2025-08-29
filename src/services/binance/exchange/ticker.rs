use std::collections::HashSet;

use anyhow::bail;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;
use tracing::error;

use crate::{
    libs::binance_api::ws_streams::{Events, StreamEvent, WebsocketStream, book_ticker_stream},
    services::binance::{
        broadcast::TICKER_BROADCAST, exchange::chain::ChainSymbol, storage::BookTickerEvent,
    },
};

#[derive(Clone)]
pub struct TickerBuilder {
    ws_streams_url: String,
    ws_max_connections: usize,
}

impl TickerBuilder {
    pub fn new(ws_streams_url: String, ws_max_connections: usize) -> Self {
        Self {
            ws_streams_url,
            ws_max_connections,
        }
    }

    pub async fn build_order_books(
        &self,
        token: CancellationToken,
        chains: Vec<[ChainSymbol; 3]>,
    ) -> anyhow::Result<()> {
        let unique_symbols: HashSet<String> = chains
            .iter()
            .flat_map(|chain| chain.iter())
            .map(|chain_symbol| chain_symbol.symbol.symbol.to_lowercase())
            .collect();
        let symbols: Vec<String> = unique_symbols.into_iter().collect();

        let streams = symbols
            .iter()
            .map(|symbol| book_ticker_stream(symbol))
            .collect::<Vec<_>>();

        let mut tasks_set: JoinSet<anyhow::Result<()>> = JoinSet::new();
        let chunk_size = streams.len() / self.ws_max_connections;

        for chunk in streams.chunks(chunk_size) {
            let ws_url = self.ws_streams_url.clone();
            let streams_chunk = chunk.to_vec();
            let token = token.clone();

            tasks_set.spawn({
                async move {
                    let mut ws: WebsocketStream<'_, StreamEvent<_>> = WebsocketStream::new(ws_url.clone())
                        .with_callback(|event: StreamEvent<Events>| {
                            if let Events::BookTicker(event) = event.data {
                                let ticker = BookTickerEvent {
                                    update_id: event.update_id,
                                    symbol: event.symbol.clone(),
                                    best_bid_price: event.best_bid_price,
                                    best_bid_qty: event.best_bid_qty,
                                    best_ask_price: event.best_ask_price,
                                    best_ask_qty: event.best_ask_qty,
                                };

                                if let Err(e) = TICKER_BROADCAST.broadcast_event(ticker) {
                                    error!(error = ?e, symbol = ?event.symbol, "Failed to broadcast ticker price");
                                    bail!("Failed to broadcast ticker price: {e}");
                                }
                            };

                            Ok(())
                        });

                    match ws.connect_multiple(&streams_chunk).await {
                        Ok(()) => {
                            if let Err(e) = ws.handle_messages(token).await {
                                error!(error = ?e, ws_url = ?ws_url, "Error while running websocket");
                                bail!("Error while running websocket: {e}");
                            };
                        }
                        Err(e) => {
                            error!(error = ?e, ws_url = ?ws_url, "Failed to connect websocket");
                            bail!("Failed to connect websocket: {e}");
                        }
                    };

                    ws.disconnect().await;

                    Ok(())
                }
            });
        }

        loop {
            tokio::select! {
                _ = token.cancelled() => {
                    break;
                }
                result = tasks_set.join_next() => match result {
                    Some(Ok(Err(e))) => {
                        error!(error = ?e, "Failed to run task");
                        token.cancel();
                        break;
                    }
                    Some(Err(e)) => {
                        error!(error = ?e, "Failed to join task");
                        token.cancel();
                        break;
                    }
                    _ => {
                        break;
                    }
                }
            }
        }

        tasks_set.abort_all();
        Ok(())
    }
}
