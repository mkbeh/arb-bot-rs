use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

use crate::{config::Config, services::ExchangeService};

pub struct KucoinExchangeConfig {
    //
}

pub struct KucoinExchangeService {
    //
}

impl KucoinExchangeConfig {
    pub fn build(config: Config) -> Self {
        Self {}
    }
}

impl KucoinExchangeService {
    pub fn from_config(config: KucoinExchangeConfig) -> Self {
        Self {}
    }
}

#[async_trait]
impl ExchangeService for KucoinExchangeService {
    async fn start_arbitrage(&self, token: CancellationToken) -> anyhow::Result<()> {
        todo!()
    }
}
