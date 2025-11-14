use std::sync::LazyLock;

use async_trait::async_trait;
use rust_decimal::{Decimal, RoundingStrategy, prelude::FromPrimitive};
use tokio::sync::{Mutex, watch};
use tokio_util::sync::CancellationToken;
use tracing::info;
use uuid::Uuid;

use crate::services::enums::SymbolOrder;

/// Global channel for distributing order chains
pub static ORDERS_CHANNEL: LazyLock<OrdersSingleton> = LazyLock::new(|| {
    let (tx, rx) = watch::channel::<Chain>(Chain::default());
    OrdersSingleton {
        tx,
        rx: Mutex::new(rx),
    }
});

#[async_trait]
pub trait ExchangeService: Send + Sync {
    /// Starts the arbitration process.
    async fn start_arbitrage(&self, token: CancellationToken) -> anyhow::Result<()>;
}

#[async_trait]
pub trait OrderSenderService: Send + Sync {
    /// Starts the process of sending orders.
    async fn send_orders(&self, token: CancellationToken) -> anyhow::Result<()>;
}

pub struct OrdersSingleton {
    pub tx: watch::Sender<Chain>,
    pub rx: Mutex<watch::Receiver<Chain>>,
}

/// Chain of orders for arbitrage (buy/sell sequence).
#[derive(Clone, Debug, Default)]
pub struct Chain {
    pub ts: u128,
    pub chain_id: Uuid,
    pub fee_percent: Decimal,
    pub orders: Vec<Order>,
}

/// Order in a chain (buy/sell with qty/price).
#[derive(Clone, Debug)]
pub struct Order {
    pub symbol: String,
    pub symbol_order: SymbolOrder,
    pub price: Decimal,
    pub base_qty: Decimal,
    pub quote_qty: Decimal,
    pub base_increment: Decimal,
    pub quote_increment: Decimal,
}

/// Order book unit (price + qty)
pub struct OrderBookUnit {
    pub price: Decimal,
    pub qty: Decimal,
}

impl Chain {
    /// Extracts symbols from orders.
    pub fn extract_symbols(&self) -> Vec<&str> {
        self.orders.iter().map(|o| o.symbol.as_str()).collect()
    }

    /// Calculates the chain's profit taking into account the fee.
    pub fn compute_profit(&self) -> (Decimal, Decimal) {
        if self.orders.is_empty() {
            return (Decimal::ZERO, Decimal::ZERO);
        }

        let input_qty = self.orders[0].base_qty;
        let output_qty = self.orders.last().unwrap().quote_qty; // Assume last is output

        let fee_rate = self.fee_percent / Decimal::ONE_HUNDRED;
        let scale_factor = Decimal::from_usize(self.orders.len()).unwrap_or(Decimal::ONE);

        let fee = (scale_factor * (input_qty * fee_rate))
            .round_dp_with_strategy(8, RoundingStrategy::MidpointAwayFromZero);
        let profit = (output_qty - input_qty - fee)
            .round_dp_with_strategy(8, RoundingStrategy::MidpointAwayFromZero);
        let profit_percent = ((profit / input_qty) * Decimal::ONE_HUNDRED)
            .round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero);

        (profit, profit_percent)
    }

    /// Logs information about the chain.
    pub fn print_info(&self, send_orders: bool) {
        let (profit, profit_percent) = self.compute_profit();

        let orders_fmt = self
            .orders
            .iter()
            .map(|o| {
                format!(
                    "{}(base:{:.8}@quote:{:.8}@price:{:.8})",
                    o.symbol, o.base_qty, o.quote_qty, o.price
                )
            })
            .collect::<Vec<_>>()
            .join(" → ");

        info!(
            ts = self.ts,
            chain_id = %self.chain_id,
            send_orders = send_orders,
            profit = ?profit,
            profit_percent = ?profit_percent,
            fee_percent = %self.fee_percent,
            orders = ?orders_fmt,
            "📦 Received orders chain"
        );
    }
}
