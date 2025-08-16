use std::sync::LazyLock;

use async_trait::async_trait;
use rust_decimal::Decimal;
use tokio::sync::{
    Mutex,
    mpsc::{Receiver, Sender},
};
use uuid::Uuid;

use crate::services::enums::SymbolOrder;

pub static ORDERS_CHANNEL: LazyLock<OrdersSingleton> = LazyLock::new(|| {
    const BUF_SIZE: usize = 1_000;
    let (tx, rx) = tokio::sync::mpsc::channel::<Chain>(BUF_SIZE);
    OrdersSingleton {
        tx,
        rx: Mutex::new(rx),
    }
});

#[async_trait]
pub trait ExchangeService: Send + Sync {
    async fn start_arbitrage(&self) -> anyhow::Result<()>;
}

#[async_trait]
pub trait OrderSenderService: Send + Sync {
    async fn send_orders(&self, msg: Chain) -> anyhow::Result<()>;
    async fn send_orders_ws(&self) -> anyhow::Result<()>;
}

pub struct OrdersSingleton {
    pub tx: Sender<Chain>,
    pub rx: Mutex<Receiver<Chain>>,
}

#[derive(Clone, Debug)]
pub struct Chain {
    pub ts: u64,
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
