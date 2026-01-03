use std::sync::LazyLock;

use tokio::sync::{Mutex, watch};

use crate::model::orders::ChainOrders;

// Global channel for distributing order chains.
pub static ORDERS_CHANNEL: LazyLock<OrdersChannel> = LazyLock::new(|| {
    let (tx, rx) = watch::channel(ChainOrders::default());
    OrdersChannel {
        tx,
        rx: Mutex::new(rx),
    }
});

pub struct OrdersChannel {
    pub tx: watch::Sender<ChainOrders>,
    pub rx: Mutex<watch::Receiver<ChainOrders>>,
}
