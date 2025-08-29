use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

use async_trait::async_trait;
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, log::debug, warn};

use crate::{
    libs::binance_api::{
        OrderSide, OrderStatus, OrderType, TimeInForce,
        ws::{
            PlaceOrderRequest, QueryOrderRequest, WebsocketApi, WebsocketClient,
            WebsocketClientError, WebsocketConnectConfig,
        },
    },
    services::{
        ORDERS_CHANNEL, Order, binance::REQUEST_WEIGHT, enums::SymbolOrder,
        service::OrderSenderService,
    },
};

const POLL_TIMEOUT: Duration = Duration::from_secs(900);
const POLL_INTERVAL: Duration = Duration::from_secs(1);

pub struct BinanceSenderConfig {
    pub send_orders: bool,
    pub ws_url: String,
    pub api_token: String,
    pub api_secret_key: String,
}

pub struct BinanceSender {
    send_orders: bool,
    ws_url: String,
    api_token: String,
    api_secret_key: String,
}

impl BinanceSender {
    pub fn new(config: BinanceSenderConfig) -> Self {
        Self {
            send_orders: config.send_orders,
            ws_url: config.ws_url,
            api_token: config.api_token,
            api_secret_key: config.api_secret_key,
        }
    }
}

#[async_trait]
impl OrderSenderService for BinanceSender {
    async fn send_orders(&self, token: CancellationToken) -> anyhow::Result<()> {
        let ws_conf = WebsocketConnectConfig {
            ws_url: self.ws_url.clone(),
            api_key: self.api_token.clone(),
            secret_key: self.api_secret_key.clone(),
        };
        let (mut ws_writer, ws_reader) = WebsocketClient::connect(ws_conf).await?;

        let message_handler = tokio::spawn({
            let token = token.clone();
            async move { ws_reader.handle_messages(token).await }
        });

        let mut orders_rx = ORDERS_CHANNEL.rx.lock().await;

        // todo: remove it
        let flag = AtomicBool::new(false);

        loop {
            tokio::select! {
                _ = token.cancelled() => {
                    break;
                }
                message = orders_rx.recv() => {
                    if let Some(ref chain) = message {
                        info!(chain = ?chain, send_orders = ?self.send_orders, "received chain orders");

                        if !self.send_orders {
                            continue;
                        }

                        if flag.load(Ordering::Relaxed) {
                            continue;
                        } else {
                            flag.store(true, Ordering::Relaxed);
                        }

                        'outer: for order in chain.orders.iter() {
                            let place_order_request = PlaceOrderRequest{
                                symbol: order.symbol.to_owned(),
                                order_side: define_order_side(order),
                                order_type: OrderType::Limit,
                                time_in_force: Some(TimeInForce::Gtc),
                                quantity: define_order_qty(order),
                                quote_order_qty: None,
                                price: Some(order.price.to_string()),
                                new_client_order_id: None,
                                strategy_id: None,
                                strategy_type: None,
                                stop_price: None,
                                trailing_delta: None,
                                iceberg_qty: None,
                                new_order_resp_type: None,
                                self_trade_prevention_mode: None,
                                recv_window: None,
                                timestamp: None,
                                api_key: None,
                                signature: None,
                            };

                            loop {
                                let mut request_weight = REQUEST_WEIGHT.lock().await;
                                if request_weight.add(WebsocketApi::PlaceOrder.weight() as usize) {
                                    break;
                                }

                                tokio::time::sleep(Duration::from_secs(1)).await;
                            }

                            let order_id = match ws_writer.place_order(place_order_request).await {
                                Ok(response) => {
                                    info!(response = ?response, "Order placed successfully");
                                    if response.status == OrderStatus::Filled {
                                        continue;
                                    }
                                    response.order_id
                                },
                                Err(err) => {
                                    error!(error = ?err, "Error try placing order");
                                    break;
                                }
                            };

                            // Check order status
                            let query_order_request = QueryOrderRequest{
                                symbol: order.symbol.to_owned(),
                                order_id: Some(order_id),
                                orig_client_order_id: None,
                                recv_window: None,
                                timestamp: None,
                                api_key: None,
                                signature: None,
                            };

                            let start_time = Instant::now();

                            loop {
                                let can_processed = {
                                    let mut request_weight = REQUEST_WEIGHT.lock().await;
                                    request_weight.add(WebsocketApi::QueryOrder.weight() as usize)
                                };

                                if !can_processed {
                                    tokio::time::sleep(Duration::from_secs(1)).await;
                                    continue;
                                };

                                match ws_writer.query_order(query_order_request.clone()).await {
                                    Ok(response) => {
                                        match response.status {
                                            OrderStatus::Filled => break,
                                            _ => {}
                                        }
                                    },
                                    Err(e) => {
                                        error!(error = ?e, "Failed to query order status");
                                        if e.downcast_ref::<WebsocketClientError>()
                                            .map(|e| matches!(e, WebsocketClientError::Timeout(_)))
                                            .unwrap_or(false)
                                        {
                                            continue;
                                        }
                                        break 'outer;
                                    }
                                }

                                if start_time.elapsed() >= POLL_TIMEOUT {
                                    // todo: sell everything at market price
                                    warn!(chain_id = ?chain.chain_id, order_id = ?order_id, "Timed out trying to poll order status");
                                    break 'outer;
                                }

                                tokio::time::sleep(POLL_INTERVAL).await;
                            }
                        }
                    }
                }
            }
        }

        let _ = message_handler.await;
        Ok(())
    }
}

fn define_order_side(order: &Order) -> OrderSide {
    match order.symbol_order {
        SymbolOrder::Asc => OrderSide::Sell,
        SymbolOrder::Desc => OrderSide::Buy,
    }
}

fn define_order_qty(order: &Order) -> Option<String> {
    match order.symbol_order {
        SymbolOrder::Asc => Some(order.base_qty.to_string()),
        SymbolOrder::Desc => Some(order.quote_qty.to_string()),
    }
}
