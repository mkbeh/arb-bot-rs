use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

use crate::{config::Config, services::OrderSenderService};

pub struct KucoinSenderConfig {
    //
}

pub struct KucoinSenderService {
    //
}

impl From<&Config> for KucoinSenderConfig {
    fn from(config: &Config) -> Self {
        Self {}
    }
}

impl KucoinSenderService {
    pub fn from_config(config: KucoinSenderConfig) -> anyhow::Result<Self> {
        Ok(Self {})
    }
}

#[async_trait]
impl OrderSenderService for KucoinSenderService {
    async fn send_orders(&self, token: CancellationToken) -> anyhow::Result<()> {
        Ok(())
    }
}
