use std::sync::LazyLock;

use async_trait::async_trait;
use rust_decimal::Decimal;
use tokio::sync::{
    Mutex,
    mpsc::{Receiver, Sender},
};

use crate::services::enums::SymbolOrder;

pub static ORDERS_CHANNEL: LazyLock<OrdersSingleton> = LazyLock::new(|| {
    const BUF_SIZE: usize = 1_000;
    let (tx, rx) = tokio::sync::mpsc::channel::<Vec<Order>>(BUF_SIZE);
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
    async fn send_orders(&self, msg: Vec<Order>) -> anyhow::Result<()>;
}

pub struct OrdersSingleton {
    pub tx: Sender<Vec<Order>>,
    pub rx: Mutex<Receiver<Vec<Order>>>,
}

#[derive(Clone, Debug)]
pub struct Order {
    pub symbol: String,
    pub symbol_order: SymbolOrder,
    pub price: Decimal,
    pub base_qty: Decimal,
    pub quote_qty: Decimal,
}
