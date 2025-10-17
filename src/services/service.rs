use std::sync::LazyLock;

use async_trait::async_trait;
use rust_decimal::{Decimal, RoundingStrategy};
use tokio::sync::{Mutex, watch};
use tokio_util::sync::CancellationToken;
use tracing::{info, info_span};
use uuid::Uuid;

use crate::services::enums::SymbolOrder;

pub static ORDERS_CHANNEL: LazyLock<OrdersSingleton> = LazyLock::new(|| {
    let (tx, rx) = watch::channel::<Chain>(Chain::default());
    OrdersSingleton {
        tx,
        rx: Mutex::new(rx),
    }
});

#[async_trait]
pub trait ExchangeService: Send + Sync {
    async fn start_arbitrage(&self, token: CancellationToken) -> anyhow::Result<()>;
}

#[async_trait]
pub trait OrderSenderService: Send + Sync {
    async fn send_orders(&self, token: CancellationToken) -> anyhow::Result<()>;
}

pub struct OrdersSingleton {
    pub tx: watch::Sender<Chain>,
    pub rx: Mutex<watch::Receiver<Chain>>,
}

#[derive(Clone, Debug, Default)]
pub struct Chain {
    pub ts: u128,
    pub chain_id: Uuid,
    pub orders: Vec<Order>,
}

#[derive(Clone, Debug)]
pub struct Order {
    pub symbol: String,
    pub symbol_order: SymbolOrder,
    pub price: Decimal,
    pub base_qty: Decimal,
    pub quote_qty: Decimal,
}

impl Chain {
    pub fn extract_symbols(&self) -> Vec<&str> {
        self.orders.iter().map(|o| o.symbol.as_str()).collect()
    }
}

pub fn print_chain_info(chain: &Chain, send_orders: bool) {
    let span = info_span!(
        "chain_received",
        ts = chain.ts,
        chain_id = %chain.chain_id,
        send_orders = send_orders
    );
    let _enter = span.enter();

    let (profit, profit_percent) = {
        let input_qty = chain.orders[0].base_qty;
        let output_qty = chain.orders[2].quote_qty;
        let profit = (output_qty - input_qty)
            .round_dp_with_strategy(8, RoundingStrategy::MidpointAwayFromZero);
        (
            profit,
            ((profit / input_qty) * Decimal::ONE_HUNDRED).round_dp(2),
        )
    };

    info!(
        profit = ?profit,
        profit_percent = ?profit_percent,
        orders = ?chain.orders.iter().map(|o| format!("{}(base:{:.8}@quote:{:.8}@price:{:.8})", o.symbol, o.base_qty, o.quote_qty, o.price)).collect::<Vec<_>>().join(" → "),
        "📦 Received orders chain"
    );
}
