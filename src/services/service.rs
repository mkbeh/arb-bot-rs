use std::sync::LazyLock;

use async_trait::async_trait;
use rust_decimal::Decimal;
use tokio::sync::{Mutex, broadcast};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::services::enums::SymbolOrder;

pub static ORDERS_CHANNEL: LazyLock<OrdersSingleton> = LazyLock::new(|| {
    let (tx, rx) = broadcast::channel::<Chain>(1);
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
    pub tx: broadcast::Sender<Chain>,
    pub rx: Mutex<broadcast::Receiver<Chain>>,
}

#[derive(Clone, Debug)]
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
