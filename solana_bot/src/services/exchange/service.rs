use async_trait::async_trait;
use engine::{Exchange, service::traits::ArbitrageService};
use tokio_util::sync::CancellationToken;

use crate::Config;

/// Core service for exchange arbitrage operations.
pub struct ExchangeService {
    //
}

impl ExchangeService {
    pub async fn from_config(config: &Config) -> anyhow::Result<Self> {
        Ok(Self {})
    }
}

impl Exchange for ExchangeService {}

#[async_trait]
impl ArbitrageService for ExchangeService {
    async fn start(&self, token: CancellationToken) -> anyhow::Result<()> {
        Ok(())
    }
}
