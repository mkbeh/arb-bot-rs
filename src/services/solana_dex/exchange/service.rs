use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

use crate::{config::Config, services::Exchange};

/// Core service for arbitrage operations.
pub struct ExchangeService {
    // todo
}

impl ExchangeService {
    pub fn from_config(config: &Config) -> anyhow::Result<Self> {
        todo!()
    }
}

#[async_trait]
impl Exchange for ExchangeService {
    /// Starts the arbitrage process.
    async fn start_arbitrage(&self, token: CancellationToken) -> anyhow::Result<()> {
        todo!()
    }
}
