use std::time::Duration;

use anyhow::bail;
use async_trait::async_trait;
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;
use tracing::{error, info};
use uuid::Uuid;

use crate::{
    config::Config,
    libs::binance_api::{
        OrderSide, OrderStatus, OrderType, TimeInForce, ws,
        ws::{
            PlaceOrderRequest, PlaceOrderResponse, QueryOrderRequest, QueryOrderResponse,
            WebsocketApi, WebsocketClientError, WebsocketWriter, connect_ws,
        },
    },
    services::{
        ORDERS_CHANNEL, Order,
        binance::{
            REQUEST_WEIGHT,
            metrics::{METRICS, ProcessChainStatus},
        },
        enums::SymbolOrder,
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

pub struct BinanceSenderService {
    send_orders: bool,
    order_lifetime: Duration,
    process_chain_interval: Duration,
    poll_interval: Duration,
    ws_url: String,
    api_token: String,
    api_secret_key: String,
}

impl BinanceSenderConfig {
    pub fn build(config: Config) -> Self {
        Self {
            send_orders: config.settings.send_orders,
            order_lifetime_secs: config.settings.order_lifetime,
            ws_url: config.binance.ws_url,
            api_token: config.binance.api_token.clone(),
            api_secret_key: config.binance.api_secret_key.clone(),
        }
    }
}

impl BinanceSenderService {
    pub fn from_config(config: BinanceSenderConfig) -> Self {
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
impl OrderSenderService for BinanceSenderService {
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

        // Get the initial value from watch channel
        _ = orders_rx.borrow().clone();

        loop {
            tokio::select! {
                _ = token.cancelled() => {
                    break;
                }

                _ = orders_rx.changed() => {
                    let chain = orders_rx.borrow().clone();
                    chain.print_info(self.send_orders);

                    METRICS.increment_profit_orders(&chain.extract_symbols(), ProcessChainStatus::New);

                    if !self.send_orders {
                        continue;
                    }

                    if last_chain_exec_ts.is_some_and(|t| t.elapsed() < self.process_chain_interval) {
                        continue;
                    }

                    for (i, order) in chain.orders.iter().enumerate() {
                        if let Err(e) = self.process_order(i, chain.chain_id, order, &mut ws_writer).await {
                            error!(error = ?e, "Error processing order");
                            METRICS.increment_profit_orders(&chain.extract_symbols(), ProcessChainStatus::Cancelled);
                            break
                        };
                    }

                    last_chain_exec_ts = Some(Instant::now());
                    METRICS.increment_profit_orders(&chain.extract_symbols(), ProcessChainStatus::Filled);
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

impl BinanceSenderService {
    async fn process_order(
        &self,
        idx: usize,
        chain_id: Uuid,
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
                print_place_order(idx, chain_id, &response);
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
                        print_query_order(chain_id, &response);
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

fn print_place_order(idx: usize, chain_id: Uuid, response: &PlaceOrderResponse) {
    let status_emoji = if response.status == OrderStatus::Filled {
        "✅"
    } else {
        "⏳"
    };
    info!(
        chain_id = chain_id.to_string(),
        order_index = idx + 1,
        symbol = %response.symbol,
        order_id = response.order_id,
        client_order_id = %response.client_order_id,
        transact_time_ms = response.transact_time,
        price = ?response.price,
        orig_qty = ?response.orig_qty,
        executed_qty = ?response.executed_qty,
        cummulative_quote_qty = ?response.cummulative_quote_qty,
        status = %response.status,
        order_type = %response.order_type,
        order_side = %response.order_side,
        fills_count = response.fills.len(),
        "{} Order placed successfully",
        status_emoji
    );
}

fn print_query_order(chain_id: Uuid, response: &QueryOrderResponse) {
    info!(
        chain_id = chain_id.to_string(),
        symbol = %response.symbol,
        order_id = response.order_id,
        client_order_id = %response.client_order_id,
        price = ?response.price,
        orig_qty = ?response.orig_qty,
        executed_qty = ?response.executed_qty,
        cummulative_quote_qty = ?response.cummulative_quote_qty,
        status = %response.status,
        update_time_ms = response.update_time,
        "✅ Order filled successfully"
    );
}
