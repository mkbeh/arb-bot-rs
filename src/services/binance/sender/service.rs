//! Binance order sender service for executing arbitrage chains.

use std::time::Duration;

use anyhow::Context;
use async_trait::async_trait;
use rust_decimal::Decimal;
use tokio::{sync::oneshot, task::JoinSet, time::Instant};
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

use crate::{
    config::Config,
    libs::binance_api::{
        OrderSide, OrderType, ws,
        ws::{PlaceOrderRequest, WebsocketApi, WebsocketWriter, connect_ws},
    },
    services::{
        Chain, ORDERS_CHANNEL, Order,
        enums::{ChainStatus, SymbolOrder},
        metrics::METRICS,
        service::Sender,
        weight::REQUEST_WEIGHT,
    },
};

/// Configuration for the Binance sender service.
pub struct SenderConfig {
    pub send_orders: bool,
    pub ws_url: String,
    pub api_token: String,
    pub api_secret_key: String,
    pub process_chain_interval_secs: u64,
}

impl From<&Config> for SenderConfig {
    fn from(config: &Config) -> Self {
        Self {
            send_orders: config.settings.send_orders,
            ws_url: config.binance.ws_url.clone(),
            api_token: config.binance.api_token.clone(),
            api_secret_key: config.binance.api_secret_key.clone(),
            process_chain_interval_secs: 10,
        }
    }
}

/// Service for sending and polling Binance orders from arbitrage chains.
#[derive(Clone)]
pub struct SenderService {
    send_orders: bool,
    process_chain_interval: Duration,
    ws_url: String,
    api_token: String,
    api_secret_key: String,
}

#[async_trait]
impl Sender for SenderService {
    async fn send_orders(&self, token: CancellationToken) -> anyhow::Result<()> {
        let mut tasks: JoinSet<anyhow::Result<()>> = JoinSet::new();

        tasks.spawn({
            let this = self.clone();
            let token = token.clone();
            async move { this.receive_and_send_orders(token).await }
        });

        while let Some(result) = tasks.join_next().await {
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

impl SenderService {
    pub fn from_config(config: SenderConfig) -> anyhow::Result<Self> {
        Ok(Self {
            send_orders: config.send_orders,
            process_chain_interval: Duration::from_secs(config.process_chain_interval_secs),
            ws_url: config.ws_url,
            api_token: config.api_token,
            api_secret_key: config.api_secret_key,
        })
    }

    /// Main loop for receiving arbitrage chains and sending corresponding orders.
    /// Monitors a watch channel for new chains, processes them with rate limiting,
    /// and handles WebSocket messages in parallel.
    async fn receive_and_send_orders(&self, token: CancellationToken) -> anyhow::Result<()> {
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

                _ = orders_rx.changed() => {
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
                        continue;
                    }

                    chain.print_info(self.send_orders);
                    METRICS.record_chain_status(&chain_symbols, ChainStatus::New);

                    if let Err(e) =
                        self.process_chain_orders(&mut ws_writer, chain.clone()).await
                    {
                        METRICS.record_chain_status(&chain_symbols, ChainStatus::Cancelled);
                        error!(error = ?e, "❌📦 Error processing chain orders");
                        break;
                    }

                    last_chain_exec_ts = Some(Instant::now());
                    METRICS.record_chain_status(&chain_symbols, ChainStatus::Filled);
                }

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
        ws_writer.disconnect().await;

        Ok(())
    }

    /// Sets up the WebSocket connection and spawns a message handler task.
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

    /// Processes an entire arbitrage chain by sequentially placing orders.
    /// Computes quantities based on previous fills and logs the final profit.
    async fn process_chain_orders(
        &self,
        ws_writer: &mut WebsocketWriter,
        chain: Chain,
    ) -> anyhow::Result<()> {
        let mut filled_sizes = Vec::with_capacity(chain.orders.len());
        let mut last_filled_qty: Option<Decimal> = None;

        for (idx, order) in chain.orders.iter().enumerate() {
            let (base_qty, quote_qty) = if let Some(filled_size) = last_filled_qty {
                Self::compute_order_quantities(order, filled_size)
            } else {
                define_order_quantities(order)
            };

            let request = Self::build_place_order_request(order, base_qty, quote_qty);
            let (filled_size, stats_filled_size) =
                Self::process_order_request(ws_writer, chain.clone(), idx, request).await?;

            last_filled_qty = Some(filled_size);
            filled_sizes.push(stats_filled_size);
        }

        // Compute and log chain profit
        let profit = Self::compute_chain_profit(&filled_sizes)
            .with_context(|| format!("Failed to calculate profit for chain {}", chain.chain_id))?;

        info!(
            chain_id = %chain.chain_id,
            first_size = %filled_sizes.first().unwrap_or(&Decimal::ZERO),
            last_size = %filled_sizes.last().unwrap_or(&Decimal::ZERO),
            profit = %profit,
            "✅ Chain completed: profit calculated"
        );

        Ok(())
    }

    /// Places a single order via WebSocket and extracts filled quantities.
    /// Handles special logic for the first order in ascending chains.
    async fn process_order_request(
        ws_writer: &mut WebsocketWriter,
        chain: Chain,
        order_idx: usize,
        request: PlaceOrderRequest,
    ) -> anyhow::Result<(Decimal, Decimal)> {
        Self::wait_for_weight(WebsocketApi::PlaceOrder).await?;
        let response = ws_writer
            .place_order(request.clone())
            .await
            .with_context(|| "Failed to place order")?;

        let executed_qty = response.executed_qty;
        let cummulative_quote_qty = response.cummulative_quote_qty;

        let filled_qty = match chain.orders[order_idx].symbol_order {
            SymbolOrder::Asc => cummulative_quote_qty,
            SymbolOrder::Desc => executed_qty,
        };

        let stats_filled_qty =
            if order_idx == 0 && matches!(chain.orders[order_idx].symbol_order, SymbolOrder::Asc) {
                executed_qty
            } else {
                cummulative_quote_qty
            };

        info!(
            chain_id = %chain.chain_id,
            order_index = order_idx + 1,
            symbol = %request.symbol,
            order_id = response.order_id,
            client_order_id = %response.client_order_id,
            order_type = %request.order_type,
            order_side = %request.order_side,
            stats_filled_qty = %stats_filled_qty,
            filled_qty = %filled_qty,
            "✅ Order filled successfully",
        );

        Ok((filled_qty, stats_filled_qty))
    }

    /// Computes order quantities based on the previous filled size and symbol direction.
    fn compute_order_quantities(
        order: &Order,
        filled_size: Decimal,
    ) -> (Option<String>, Option<String>) {
        let size = (filled_size / order.base_increment).round() * order.base_increment;

        match order.symbol_order {
            SymbolOrder::Asc => (Some(size.to_string()), None),
            SymbolOrder::Desc => (None, Some(size.to_string())),
        }
    }

    /// Waits for available API weight before proceeding with a request.
    /// Uses a global mutex to track and increment weights.
    async fn wait_for_weight(api: WebsocketApi) -> anyhow::Result<()> {
        loop {
            if REQUEST_WEIGHT.lock().await.add(api.weight() as usize) {
                break;
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
        Ok(())
    }

    /// Builds a `PlaceOrderRequest` payload from order details and quantities.
    fn build_place_order_request(
        order: &Order,
        base_qty: Option<String>,
        quote_qty: Option<String>,
    ) -> PlaceOrderRequest {
        PlaceOrderRequest {
            symbol: order.symbol.clone(),
            order_side: define_order_side(order),
            order_type: OrderType::Market,
            time_in_force: None,
            quantity: base_qty,
            quote_order_qty: quote_qty,
            price: None,
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

    /// Computes the profit for a completed chain as the difference between last and first filled
    /// sizes.
    fn compute_chain_profit(filled_sizes: &[Decimal]) -> anyhow::Result<Decimal> {
        let first_size = filled_sizes
            .first()
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("No orders processed: filled_sizes is empty"))?;
        let last_size = filled_sizes
            .last()
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("No completed orders: filled_sizes is empty"))?;

        let profit = last_size - first_size;
        Ok(profit)
    }
}

/// Determines the order side based on the symbol order direction.
fn define_order_side(order: &Order) -> OrderSide {
    match order.symbol_order {
        SymbolOrder::Asc => OrderSide::Sell,
        SymbolOrder::Desc => OrderSide::Buy,
    }
}

/// Defines initial quantities for the first order in a chain.
fn define_order_quantities(order: &Order) -> (Option<String>, Option<String>) {
    match order.symbol_order {
        SymbolOrder::Asc => (Some(order.base_qty.to_string()), None),
        SymbolOrder::Desc => (None, Some(order.base_qty.to_string())),
    }
}
