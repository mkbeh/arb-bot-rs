use async_trait::async_trait;

use crate::{
    libs::binance_api::{Account, Trade},
    services::{Order, service::OrderSenderService},
};

pub struct BinanceSenderConfig {
    pub account_api: Account,
    pub trade_api: Trade,
}

pub struct BinanceSender {
    pub account_api: Account,
    pub trade_api: Trade,
}

impl BinanceSender {
    pub fn new(config: BinanceSenderConfig) -> BinanceSender {
        BinanceSender {
            account_api: config.account_api,
            trade_api: config.trade_api,
        }
    }
}

#[async_trait]
impl OrderSenderService for BinanceSender {
    async fn send_orders(&self, msg: Vec<Order>) -> anyhow::Result<()> {
        println!("Sending orders: {:?}", msg);

        Ok(())
    }
}
