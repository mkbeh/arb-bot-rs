use std::collections::HashSet;

use anyhow::bail;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;
use tracing::error;

use crate::{
    libs::kucoin_api::{
        BaseInfo,
        stream::{Events, MessageEvents, OrderRow, WebsocketStream, order_book_increment_topic},
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
            let topic_symbols = order_book_increment_topic(chunk);
            let api_token = api_token.clone();
            let token = token.clone();

            tasks_set.spawn(async move {
                let mut ws: WebsocketStream<'_, Events> = WebsocketStream::new(
                    ws_endpoint.clone(),
                    ping_interval,
                )
                    .with_callback(|event: Events| {
                        if let Events::Message(event) = event {
                            let MessageEvents::IncrementOrderBook(message) = *event;
                            let symbol = &message.symbol;
                            let mut changes = BookTickerEventChanges::new(symbol);

                            if let Some(bid_row) = message.latest_bid() {
                                changes.bid = Some(create_ticker_event(symbol, bid_row));
                            }
                            if let Some(ask_row) = message.latest_ask() {
                                changes.ask = Some(create_ticker_event(symbol, ask_row));
                            }

                            if changes != BookTickerEventChanges::default()
                                && let Err(e) = TICKER_BROADCAST.broadcast_event(changes)
                            {
                                error!(
                                    error = ?e,
                                    symbol = %symbol,
                                    "Failed to broadcast changes event"
                                );
                                bail!("Failed to broadcast changes event: {e}");
                            };

                            METRICS.increment_book_ticker_events(symbol);
                        }
                        Ok(())
                    });

                match ws.connect(topic_symbols, api_token, false).await {
                    Ok(()) => {
                        if let Err(e) = ws.handle_messages(token).await {
                            error!(error = ?e, ws_url = ?ws_endpoint, "Error while running websocket");
                            bail!("Error while running websocket: {e}");
                        };
                    }
                    Err(e) => {
                        error!(error = ?e, ws_url = ?ws_endpoint, "Failed to connect websocket");
                        bail!("Failed to connect websocket: {e}");
                    }
                }

                ws.disconnect().await;

                Ok(())
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
}

fn create_ticker_event(symbol: &str, row: OrderRow) -> BookTickerEvent {
    let OrderRow(price, qty, sequence_id) = row;
    BookTickerEvent {
        symbol: symbol.to_string(),
        sequence_id,
        price,
        qty,
    }
}
