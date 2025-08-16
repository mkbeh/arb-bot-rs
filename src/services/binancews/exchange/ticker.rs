use std::sync::{Arc, atomic::AtomicBool};

use anyhow::bail;
use tokio::task::JoinSet;
use tracing::error;

use crate::{
    libs::binance_api::{
        General,
        ws_streams::{Events, StreamEvent, WebsocketStream, book_ticker_stream},
    },
    services::binancews::storage::{BookTickerEvent, BookTickerStore},
};

#[derive(Clone)]
pub struct TickerBuilder {
    ws_url: String,
    general_api: General,
}

impl TickerBuilder {
    pub fn new(ws_url: String, general_api: General) -> Self {
        Self {
            ws_url,
            general_api,
        }
    }

    pub async fn build_tickers_order_books(&self, store: Arc<BookTickerStore>) {
        let exchange_info = match self.general_api.exchange_info().await {
            Ok(exchange_info) => exchange_info,
            Err(e) => {
                error!(error = ?e, "failed get exchange info");
                return;
            }
        };

        let symbols = exchange_info
            .symbols
            .into_iter()
            .map(|symbol| symbol.symbol.to_lowercase())
            .collect::<Vec<_>>();

        let streams = symbols
            .into_iter()
            .map(|symbol| book_ticker_stream(&symbol))
            .collect::<Vec<_>>();

        let mut tasks_set = JoinSet::new();
        let chunk_size = 100;

        for chunk in streams.chunks(chunk_size) {
            let store = store.clone();
            let ws_url = self.ws_url.clone();
            let streams_chunk = chunk.to_vec();

            tasks_set.spawn({
                async move {
                    let mut ws: WebsocketStream<'_, StreamEvent<_>> = WebsocketStream::new(ws_url)
                        .with_callback(|event: StreamEvent<Events>| {
                            if let Events::BookTicker(ticker) = event.data {
                                store.update(BookTickerEvent {
                                    update_id: ticker.update_id,
                                    symbol: ticker.symbol,
                                    best_bid_price: ticker.best_bid_price,
                                    best_bid_qty: ticker.best_bid_qty,
                                    best_ask_price: ticker.best_ask_price,
                                    best_ask_qty: ticker.best_ask_qty,
                                })
                            };

                            Ok(())
                        });

                    ws.connect_multiple(&streams_chunk).await.unwrap();

                    let flag = AtomicBool::new(true);
                    if let Err(e) = ws.run(&flag).await {
                        println!("Error while running websocket: {:?}", e);
                    };

                    // ws.disconnect().await.unwrap();
                }
            });
        }

        while let Some(result) = tasks_set.join_next().await {
            if let Err(e) = result {
                tasks_set.abort_all();
                error!(error = ?e, "failed get tickers order book");
                return;
            }
        }
    }
}
