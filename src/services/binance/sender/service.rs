//! Binance order sender service for executing arbitrage chains.

use std::time::Duration;

use anyhow::{Context, bail};
use async_trait::async_trait;
use tokio::{
    sync::{oneshot, watch},
    time::Instant,
};
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
        Chain, ORDERS_CHANNEL, Order,
        enums::{ChainStatus, SymbolOrder},
        metrics::METRICS,
        service::OrderSenderService,
        weight::REQUEST_WEIGHT,
    },
};

/// Configuration for the Binance sender service.
pub struct BinanceSenderConfig {
    pub send_orders: bool,
    pub order_lifetime_secs: u64,
    pub ws_url: String,
    pub api_token: String,
    pub api_secret_key: String,
    pub process_chain_interval_secs: u64,
    pub poll_interval_secs: u64,
}

impl From<&Config> for BinanceSenderConfig {
    fn from(config: &Config) -> Self {
        Self {
            send_orders: config.settings.send_orders,
            order_lifetime_secs: config.settings.order_lifetime,
            ws_url: config.binance.ws_url.clone(),
            api_token: config.binance.api_token.clone(),
            api_secret_key: config.binance.api_secret_key.clone(),
            process_chain_interval_secs: 60,
            poll_interval_secs: 5,
        }
    }
}

/// Service for sending and polling Binance orders from arbitrage chains.
pub struct BinanceSenderService {
    send_orders: bool,
    order_lifetime: Duration,
    process_chain_interval: Duration,
    poll_interval: Duration,
    ws_url: String,
    api_token: String,
    api_secret_key: String,
}

#[async_trait]
impl OrderSenderService for BinanceSenderService {
    /// Starts listening for chains and sending orders.
    async fn send_orders(&self, token: CancellationToken) -> anyhow::Result<()> {
        let (mut ws_writer, message_handler, mut message_done_rx) =
            self.setup_websocket(token.clone()).await?;

        let mut orders_rx = ORDERS_CHANNEL.rx.lock().await;
        let mut last_chain_exec_ts: Option<Instant> = None;

        // Get the initial value from watch channel
        _ = orders_rx.borrow().clone();

        loop {
            tokio::select! {
                _ = token.cancelled() => {
                    break;
                }
                _ = orders_rx.changed() => self.process_chain_orders(&mut orders_rx, &mut ws_writer, &mut last_chain_exec_ts).await?,
                result = &mut message_done_rx => match result {
                    Ok(Err(e)) => {
                        error!("Message handler failed: {e}");
                        break;
                    }
                    Err(_) => {
                        error!("Message handler channel closed unexpectedly");
                        break;
                    }
                    _ => break,
                }
            }
        }

        message_handler.abort();
        let _ = message_handler.await;

        Ok(())
    }
}

impl BinanceSenderService {
    pub fn from_config(config: BinanceSenderConfig) -> anyhow::Result<Self> {
        Ok(Self {
            send_orders: config.send_orders,
            order_lifetime: Duration::from_secs(config.order_lifetime_secs),
            process_chain_interval: Duration::from_secs(config.process_chain_interval_secs),
            poll_interval: Duration::from_secs(config.poll_interval_secs),
            ws_url: config.ws_url,
            api_token: config.api_token,
            api_secret_key: config.api_secret_key,
        })
    }

    /// Sets up WebSocket connection and spawns reader handler.
    async fn setup_websocket(
        &self,
        token: CancellationToken,
    ) -> anyhow::Result<(
        WebsocketWriter,
        tokio::task::JoinHandle<()>,
        oneshot::Receiver<anyhow::Result<()>>,
    )> {
        let (ws_writer, ws_reader) = connect_ws(ws::ConnectConfig::new(
            self.ws_url.clone(),
            self.api_token.clone(),
            self.api_secret_key.clone(),
        ))
        .await
        .context("Failed to connect WS")?;

        let (message_done_tx, message_done_rx) = oneshot::channel();

        let message_handler = tokio::spawn({
            let token = token.clone();
            async move {
                let result = ws_reader.handle_messages(token).await;
                let _ = message_done_tx.send(result);
            }
        });

        Ok((ws_writer, message_handler, message_done_rx))
    }

    /// Processes a new chain from the watch receiver.
    async fn process_chain_orders(
        &self,
        orders_rx: &mut watch::Receiver<Chain>,
        ws_writer: &mut WebsocketWriter,
        last_chain_exec_ts: &mut Option<Instant>,
    ) -> anyhow::Result<()> {
        let chain = orders_rx.borrow().clone();
        let chain_symbols = chain.extract_symbols();

        if !self.send_orders {
            chain.print_info(self.send_orders);
            return Ok(());
        }

        if last_chain_exec_ts
            .as_ref()
            .is_some_and(|t| t.elapsed() < self.process_chain_interval)
        {
            return Ok(());
        }

        chain.print_info(self.send_orders);
        METRICS.add_chain_status(&chain_symbols, ChainStatus::New);

        for (i, order) in chain.orders.iter().enumerate() {
            if let Err(e) = self
                .process_order(i, chain.chain_id, order, ws_writer)
                .await
            {
                error!(error = ?e, "❌📦 Error processing order");
                METRICS.add_chain_status(&chain_symbols, ChainStatus::Cancelled);
                return Ok(()); // Не bail, чтобы продолжить другие chains
            }
        }

        *last_chain_exec_ts = Some(Instant::now());
        METRICS.add_chain_status(&chain_symbols, ChainStatus::Filled);

        Ok(())
    }

    /// Processes a single order.
    async fn process_order(
        &self,
        idx: usize,
        chain_id: Uuid,
        order: &Order,
        ws_writer: &mut WebsocketWriter,
    ) -> anyhow::Result<()> {
        self.wait_for_weight(WebsocketApi::PlaceOrder).await?;

        let place_order_request = self.build_place_order_request(order);
        let response = ws_writer
            .place_order(place_order_request)
            .await
            .context("Failed to place order")?;

        print_place_order(idx, chain_id, &response);

        if response.status == OrderStatus::Filled {
            return Ok(());
        }

        self.poll_order_status(ws_writer, order.symbol.clone(), response.order_id, chain_id)
            .await
    }

    /// Waits for available request weight before proceeding.
    async fn wait_for_weight(&self, api: WebsocketApi) -> anyhow::Result<()> {
        loop {
            if REQUEST_WEIGHT.lock().await.add(api.weight() as usize) {
                break;
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
        Ok(())
    }

    /// Polls order status until FILLED or lifetime exceeded.
    async fn poll_order_status(
        &self,
        ws_writer: &mut WebsocketWriter,
        symbol: String,
        order_id: u64,
        chain_id: Uuid,
    ) -> anyhow::Result<()> {
        let start_time = Instant::now();

        loop {
            if start_time.elapsed() >= self.order_lifetime {
                bail!("Timed out polling order status");
            }

            self.wait_for_weight(WebsocketApi::QueryOrder).await?;

            let query_request = QueryOrderRequest {
                symbol: symbol.clone(),
                order_id: Some(order_id),
                orig_client_order_id: None,
                recv_window: None,
                timestamp: None,
                api_key: None,
                signature: None,
            };

            match ws_writer.query_order(query_request).await {
                Ok(response) => {
                    if response.status == OrderStatus::Filled {
                        print_query_order(chain_id, &response);
                        return Ok(());
                    }
                }
                Err(e) => {
                    if let Some(WebsocketClientError::Timeout(_)) = e.downcast_ref() {
                        // Continue on timeout
                    } else {
                        return Err(e.context("Failed to query order status"));
                    }
                }
            }

            tokio::time::sleep(self.poll_interval).await;
        }
    }

    fn build_place_order_request(&self, order: &Order) -> PlaceOrderRequest {
        PlaceOrderRequest {
            symbol: order.symbol.clone(),
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
        }
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
