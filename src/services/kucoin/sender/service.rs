use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

use crate::{
    config::Config,
    services::{ORDERS_CHANNEL, OrderSenderService, enums::ChainStatus, metrics::METRICS},
};

pub struct KucoinSenderConfig {
    pub send_orders: bool,
}

pub struct KucoinSenderService {
    pub send_orders: bool,
}

impl From<&Config> for KucoinSenderConfig {
    fn from(config: &Config) -> Self {
        Self {
            send_orders: config.settings.send_orders,
        }
    }
}

impl KucoinSenderService {
    pub fn from_config(config: KucoinSenderConfig) -> anyhow::Result<Self> {
        Ok(Self {
            send_orders: config.send_orders,
        })
    }
}

#[async_trait]
impl OrderSenderService for KucoinSenderService {
    async fn send_orders(&self, token: CancellationToken) -> anyhow::Result<()> {
        let mut orders_rx = ORDERS_CHANNEL.rx.lock().await;

        // Get the initial value from watch channel
        _ = orders_rx.borrow().clone();

        loop {
            tokio::select! {
                _ = token.cancelled() => {
                    break;
                }

                _ = orders_rx.changed() => {
                    let chain = orders_rx.borrow().clone();
                    chain.print_info(self.send_orders);

                    METRICS.increment_profit_orders(&chain.extract_symbols(), ChainStatus::New);

                    if !self.send_orders {
                        continue;
                    }
                }
            }
        }

        Ok(())
    }
}
