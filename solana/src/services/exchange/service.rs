use async_trait::async_trait;
use engine::{Exchange, service::traits::ArbitrageService};
use tokio_util::sync::CancellationToken;

use crate::Config;

/// Core service for exchange arbitrage operations.
pub struct ExchangeService {}

impl Exchange for ExchangeService {}

#[async_trait]
impl ArbitrageService for ExchangeService {
    async fn start(&self, _token: CancellationToken) -> anyhow::Result<()> {
        Ok(())
    }
}

impl ExchangeService {
    pub async fn from_config(_config: &Config) -> anyhow::Result<Self> {
        Ok(Self {})
    }
}
