use std::time::Duration;

use anyhow::bail;
use async_trait::async_trait;
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

use crate::{
    libs::binance_api::{
        OrderSide, OrderStatus, OrderType, TimeInForce, ws,
        ws::{
            PlaceOrderRequest, QueryOrderRequest, WebsocketApi, WebsocketClientError,
            WebsocketWriter, connect_ws,
        },
    },
    services::{
        ORDERS_CHANNEL, Order, binance::REQUEST_WEIGHT, enums::SymbolOrder,
        service::OrderSenderService,
    },
};

pub struct BinanceSenderConfig {
    pub send_orders: bool,
    pub order_lifetime_secs: u64,
    pub ws_url: String,
    pub api_token: String,
    pub api_secret_key: String,
}

pub struct BinanceSender {
    send_orders: bool,
    order_lifetime: Duration,
    process_chain_interval: Duration,
    poll_interval: Duration,
    ws_url: String,
    api_token: String,
    api_secret_key: String,
}

impl BinanceSender {
    pub fn new(config: BinanceSenderConfig) -> Self {
        Self {
            send_orders: config.send_orders,
            order_lifetime: Duration::from_secs(config.order_lifetime_secs),
            process_chain_interval: Duration::from_secs(60),
            poll_interval: Duration::from_secs(5),
            ws_url: config.ws_url,
            api_token: config.api_token,
            api_secret_key: config.api_secret_key,
        }
    }
}

#[async_trait]
impl OrderSenderService for BinanceSender {
    async fn send_orders(&self, token: CancellationToken) -> anyhow::Result<()> {
        let (mut ws_writer, ws_reader) = connect_ws(ws::ConnectConfig::new(
            self.ws_url.clone(),
            self.api_token.clone(),
            self.api_secret_key.clone(),
        ))
        .await?;

        // Create a channel to track messages handler completion
        let (message_done_tx, mut message_done_rx) = tokio::sync::oneshot::channel();

        let message_handler = tokio::spawn({
            let token = token.clone();
            async move {
                let result = ws_reader.handle_messages(token).await;
                let _ = message_done_tx.send(result);
            }
        });

        let mut orders_rx = ORDERS_CHANNEL.rx.lock().await;
        let mut last_chain_exec_ts: Option<Instant> = None;

        loop {
            tokio::select! {
                _ = token.cancelled() => {
                    break;
                }

                Ok(chain) = orders_rx.recv() => {
                    info!(chain = ?chain, send_orders = ?self.send_orders, "received chain orders");

                    if !self.send_orders {
                        continue;
                    }

                    if last_chain_exec_ts.is_some_and(|t| t.elapsed() < self.process_chain_interval) {
                        continue;
                    }


                    for order in chain.orders.iter() {
                        if let Err(e) = self.process_order(order, &mut ws_writer).await {
                            error!(error = ?e, "Error processing order");
                            break
                        };
                    }

                    last_chain_exec_ts = Some(Instant::now());
                }

                result = &mut message_done_rx => match result {
                    Ok(Err(e)) => {
                        error!("Message handler failed: {}", e);
                        break;
                    }
                    Err(_) => {
                        error!("Message handler channel closed unexpectedly");
                        break;
                    }
                    _ => {
                        break
                    }
                }
            }
        }

        message_handler.abort();
        let _ = message_handler.await;

        Ok(())
    }
}

impl BinanceSender {
    async fn process_order(
        &self,
        order: &Order,
        ws_writer: &mut WebsocketWriter,
    ) -> anyhow::Result<()> {
        loop {
            if REQUEST_WEIGHT
                .lock()
                .await
                .add(WebsocketApi::PlaceOrder.weight() as usize)
            {
                break;
            }

            tokio::time::sleep(Duration::from_secs(1)).await;
        }

        let place_order_request = PlaceOrderRequest {
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

        let (order_id, status) = match ws_writer.place_order(place_order_request).await {
            Ok(response) => {
                info!(response = ?response, "Order placed successfully");
                (response.order_id, response.status)
            }
            Err(e) => {
                bail!("Error try placing order: {e}")
            }
        };

        if status == OrderStatus::Filled {
            return Ok(());
        }

        // Check order status
        let start_time = Instant::now();

        loop {
            if start_time.elapsed() >= self.order_lifetime {
                bail!("Timed out trying to poll order status");
            }

            if !REQUEST_WEIGHT
                .lock()
                .await
                .add(WebsocketApi::QueryOrder.weight() as usize)
            {
                tokio::time::sleep(Duration::from_secs(1)).await;
                continue;
            }

            let query_order_request = QueryOrderRequest {
                symbol: order.symbol.to_owned(),
                order_id: Some(order_id),
                orig_client_order_id: None,
                recv_window: None,
                timestamp: None,
                api_key: None,
                signature: None,
            };

            match ws_writer.query_order(query_order_request.clone()).await {
                Ok(response) => {
                    if response.status == OrderStatus::Filled {
                        info!(response = ?response, "Order filled successfully");
                        break;
                    }
                }
                Err(e) => {
                    if e.downcast_ref::<WebsocketClientError>()
                        .map(|e| matches!(e, WebsocketClientError::Timeout(_)))
                        .unwrap_or(false)
                    {
                        continue;
                    }
                    bail!("Failed to query order status: {e}");
                }
            }

            tokio::time::sleep(self.poll_interval).await;
        }

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
