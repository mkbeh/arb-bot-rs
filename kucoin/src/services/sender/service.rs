use std::time::Duration;

use anyhow::{Context, anyhow, bail};
use async_trait::async_trait;
use engine::{
    ChainOrder, ChainOrders, METRICS, ORDERS_CHANNEL, REQUEST_WEIGHT, Sender,
    enums::{ChainStatus, SymbolOrder},
    service::traits::ArbitrageService,
};
use rust_decimal::{Decimal, prelude::Zero};
use tokio::{sync::mpsc, task::JoinSet, time::Instant};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::{
    Config,
    libs::{
        kucoin_client,
        kucoin_client::{
            BaseInfo, Kucoin,
            enums::{OrderSide, OrderStatus, OrderType},
            stream::{Events, MessageEvents, OrderChange, WebsocketStream, order_change_topic},
            ws,
            ws::{AddOrderRequest, WebsocketClient},
        },
    },
};

/// Service for sending and polling Kucoin orders from arbitrage chains.
#[derive(Clone)]
pub struct SenderService {
    send_orders: bool,
    process_chain_interval: Duration,
    ws_url: String,
    api_token: String,
    api_secret: String,
    api_passphrase: String,
    base_info_api: BaseInfo,
}

impl SenderService {
    pub async fn from_config(config: &Config) -> anyhow::Result<Self> {
        // Configure global request weight limit for API rate limiting.
        {
            let mut weight_lock = REQUEST_WEIGHT.lock().await;
            weight_lock.set_weight_limit(config.api_weight_limit);
        }

        let api_config = kucoin_client::ClientConfig {
            host: config.api_url.clone(),
            api_key: config.api_token.clone(),
            api_secret: config.api_secret_key.clone(),
            api_passphrase: config.api_passphrase.clone(),
            http_config: kucoin_client::HttpConfig::default(),
        };
        let base_info_api: BaseInfo =
            Kucoin::new(api_config).context("Failed to create kucoin base info api")?;

        Ok(Self {
            send_orders: config.send_orders,
            process_chain_interval: Duration::from_secs(5),
            ws_url: config.ws_private_url.clone(),
            api_token: config.api_token.clone(),
            api_secret: config.api_secret_key.clone(),
            api_passphrase: config.api_passphrase.clone(),
            base_info_api,
        })
    }
}

impl Sender for SenderService {}

#[async_trait]
impl ArbitrageService for SenderService {
    async fn start(&self, token: CancellationToken) -> anyhow::Result<()> {
        let mut tasks: JoinSet<anyhow::Result<()>> = JoinSet::new();
        let (order_change_tx, order_change_rx) = mpsc::unbounded_channel();

        // Run balance stream listener
        tasks.spawn({
            let this = self.clone();
            let token = token.clone();
            async move { this.listen_balance_stream(token, order_change_tx).await }
        });

        // Run send orders
        tasks.spawn({
            let this = self.clone();
            let token = token.clone();
            async move { this.receive_and_send_orders(token, order_change_rx).await }
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
    /// Listens to the balance stream (order changes) via WebSocket.
    /// Fetches private endpoint, connects, and forwards order changes to a channel.
    async fn listen_balance_stream(
        &self,
        token: CancellationToken,
        order_change_tx: mpsc::UnboundedSender<OrderChange>,
    ) -> anyhow::Result<()> {
        let (api_token, ws_endpoint, ping_interval) =
            match self.base_info_api.get_bullet_private().await {
                Ok(resp) => (
                    resp.data.token,
                    resp.data.instance_servers[0].endpoint.clone(),
                    resp.data.instance_servers[0].ping_interval,
                ),
                Err(err) => bail!("Error getting bullet private: {err}"),
            };

        let mut ws_client: WebsocketStream<'_, Events> =
            WebsocketStream::new(ws_endpoint.clone(), ping_interval).with_callback(|event| {
                if let Events::Message(event) = event
                    && let MessageEvents::OrderChange(ref message) = *event
                    && let Err(e) = order_change_tx.send(*message.clone())
                {
                    error!(error = ?e, "Failed to send order change");
                };

                Ok(())
            });

        match ws_client.connect(&[order_change_topic()], api_token).await {
            Ok(()) => {
                if let Err(e) = ws_client.handle_messages(token).await {
                    error!(error = ?e, ws_url = ?ws_endpoint, "Error while running websocket");
                    bail!("Error while running websocket: {e}");
                };
            }
            Err(e) => {
                error!(error = ?e, ws_url = ?ws_endpoint, "Failed to connect websocket");
                bail!("Failed to connect websocket: {e}");
            }
        }

        ws_client.disconnect().await;
        Ok(())
    }

    /// Main loop for receiving arbitrage chains and sending orders.
    /// Monitors watch channel for chains, processes with rate limiting,
    /// and integrates order change updates from receiver channel.
    async fn receive_and_send_orders(
        &self,
        token: CancellationToken,
        mut order_change_rx: mpsc::UnboundedReceiver<OrderChange>,
    ) -> anyhow::Result<()> {
        let mut ws_client = ws::connect_ws(
            ws::ConnectConfig {
                ws_url: self.ws_url.clone(),
                token: self.api_token.clone(),
                secret_key: self.api_secret.clone(),
                passphrase: self.api_passphrase.clone(),
            },
            token.clone(),
        )
        .await?;

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
                        continue;
                    }

                    if last_chain_exec_ts.is_some_and(|t| t.elapsed() < self.process_chain_interval) {
                        continue;
                    }

                    chain.print_info(self.send_orders);
                    METRICS.record_chain_status(&chain_symbols, &ChainStatus::New);

                    if let Err(e) =
                        Self::process_chain_orders(&mut ws_client, &mut order_change_rx, chain.clone()).await
                    {
                        METRICS.record_chain_status(&chain_symbols, &ChainStatus::Cancelled);
                        error!(error = ?e, "❌ [Engine] Error processing chain orders");
                        break;
                    }

                    last_chain_exec_ts = Some(Instant::now());
                    METRICS.record_chain_status(&chain_symbols, &ChainStatus::Filled);
                }
            }
        }

        ws_client.disconnect().await;
        Ok(())
    }

    /// Processes an entire arbitrage chain by sequentially placing orders.
    /// Computes quantities based on previous fills (with fee adjustment) and waits for fills via
    /// channel. Logs the final profit.
    async fn process_chain_orders(
        ws_client: &mut WebsocketClient,
        order_change_rx: &mut mpsc::UnboundedReceiver<OrderChange>,
        chain: ChainOrders,
    ) -> anyhow::Result<()> {
        let build_order_request =
            |order: &ChainOrder, size: Option<String>, funds: Option<String>| -> AddOrderRequest {
                AddOrderRequest {
                    client_oid: Uuid::new_v4().to_string(),
                    symbol: order.symbol.clone(),
                    order_type: OrderType::Market,
                    order_side: define_order_side(order),
                    size,
                    funds,
                }
            };

        let mut filled_sizes = Vec::with_capacity(chain.orders.len());
        let mut last_filled_size: Option<Decimal> = None;
        let fee_rate = chain.fee_percent / Decimal::ONE_HUNDRED;

        for (idx, order) in chain.orders.iter().enumerate() {
            let (size, funds) = if let Some(filled_size) = last_filled_size {
                Self::compute_order_quantities(order, filled_size, fee_rate)
            } else {
                define_order_quantities(order)
            };

            let request = build_order_request(order, size, funds);
            let (filled_size, stats_filled_size) =
                Self::process_order_request(ws_client, order_change_rx, &chain, idx, request)
                    .await?;

            last_filled_size = Some(filled_size);
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
            "✅ [Engine] Chain completed: profit calculated"
        );

        Ok(())
    }

    /// Places a single order and waits for fill updates via the order change channel.
    async fn process_order_request(
        ws_client: &mut WebsocketClient,
        order_change_rx: &mut mpsc::UnboundedReceiver<OrderChange>,
        chain: &ChainOrders,
        order_idx: usize,
        request: AddOrderRequest,
    ) -> anyhow::Result<(Decimal, Decimal)> {
        let (order_id, client_order_id) = match ws_client.add_order(request.clone()).await {
            Ok(response) => (response.order_id, response.client_oid),
            Err(e) => bail!(e),
        };

        let order = &chain.orders[order_idx];
        let mut stats_filled_qty = Decimal::zero();
        let mut filled_qty = Decimal::zero();

        // Waits for the order from the channel to be filled and returns the final filled_qty
        while let Some(order_change) = order_change_rx.recv().await {
            match order_change.status {
                OrderStatus::Match => {
                    filled_qty = Self::update_filled_qty(&order_change, order, filled_qty)?;
                    stats_filled_qty +=
                        Self::compute_stats_increment(&order_change, order, order_idx)?;

                    debug!(
                        symbol = ?order.symbol,
                        status = ?order_change.status,
                        filled_qty = %filled_qty,
                        stats_qty = %stats_filled_qty,
                        "Order partially filled"
                    );
                }
                OrderStatus::Done => {
                    debug!(symbol = ?order.symbol, final_qty = %filled_qty, "Order fully done");
                    break;
                }
                _ => {
                    debug!(symbol = ?order.symbol, status = ?order_change.status, "Ignored order update");
                }
            }
        }

        info!(
            chain_id = chain.chain_id.to_string(),
            order_index = order_idx + 1,
            symbol = %request.symbol,
            order_id = order_id,
            client_order_id = %client_order_id,
            order_type = %request.order_type,
            order_side = %request.order_side,
            stats_filled_qty = %stats_filled_qty,
            filled_qty = %filled_qty,
            "✅ [Engine] Order filled successfully",
        );

        Ok((filled_qty, stats_filled_qty))
    }

    /// Updates filled_qty based on order_change and order type.
    /// Returns the updated value or an error if the fields are missing.
    fn update_filled_qty(
        order_change: &OrderChange,
        order: &ChainOrder,
        current_filled: Decimal,
    ) -> anyhow::Result<Decimal> {
        if let (Some(filled_size), Some(match_price)) =
            (order_change.filled_size, order_change.match_price)
        {
            Ok(match order.symbol_order {
                SymbolOrder::Asc => current_filled + (filled_size * match_price),
                SymbolOrder::Desc => current_filled + filled_size,
            })
        } else {
            warn!(
                symbol = ?order.symbol,
                "Incomplete match data: size={:?}, price={:?}",
                order_change.filled_size,
                order_change.match_price
            );
            Ok(current_filled) // Do not update if there is no data
        }
    }

    /// Calculates the increment for stats_filled_qty
    /// based on order_change, order, and idx.
    fn compute_stats_increment(
        order_change: &OrderChange,
        order: &ChainOrder,
        order_idx: usize,
    ) -> anyhow::Result<Decimal> {
        let size = order_change
            .filled_size
            .ok_or_else(|| anyhow!("Missing filled_size for stats"))
            .with_context(|| format!("Stats calc failed for symbol: {}", order.symbol))?;

        let increment = if order_idx == 0 && matches!(order.symbol_order, SymbolOrder::Asc) {
            size
        } else {
            let price = order_change
                .match_price
                .ok_or_else(|| anyhow!("Missing match_price for stats"))
                .with_context(|| format!("Price required for stats on symbol: {}", order.symbol))?;

            size * price
        };

        Ok(increment)
    }

    /// Computes order quantities for subsequent orders, adjusting for fees.
    fn compute_order_quantities(
        order: &ChainOrder,
        filled_size: Decimal,
        fee_rate: Decimal,
    ) -> (Option<String>, Option<String>) {
        match order.symbol_order {
            SymbolOrder::Asc => {
                let size = ((filled_size * (Decimal::ONE - fee_rate)) / order.base_increment)
                    .floor()
                    * order.base_increment;
                (Some(size.to_string()), None)
            }
            SymbolOrder::Desc => {
                let size = ((filled_size * (Decimal::ONE - fee_rate)) / order.quote_increment)
                    .floor()
                    * order.quote_increment;
                (None, Some(size.to_string()))
            }
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
fn define_order_side(order: &ChainOrder) -> OrderSide {
    match order.symbol_order {
        SymbolOrder::Asc => OrderSide::Sell,
        SymbolOrder::Desc => OrderSide::Buy,
    }
}

/// Defines initial quantities for the first order in a chain.
fn define_order_quantities(order: &ChainOrder) -> (Option<String>, Option<String>) {
    match order.symbol_order {
        SymbolOrder::Asc => (Some(order.base_qty.to_string()), None),
        SymbolOrder::Desc => (None, Some(order.base_qty.to_string())),
    }
}
