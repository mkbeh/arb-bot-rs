use async_trait::async_trait;
use engine::{Sender, service::traits::ArbitrageService};
use tokio_util::sync::CancellationToken;

use crate::Config;

/// Service for sending and polling orders from arbitrage chains.
pub struct SenderService {}

impl Sender for SenderService {}

#[async_trait]
impl ArbitrageService for SenderService {
    async fn start(&self, token: CancellationToken) -> anyhow::Result<()> {
        token.cancelled().await;
        Ok(())
    }
}

impl SenderService {
    pub async fn from_config(_config: &Config) -> anyhow::Result<Self> {
        Ok(Self {})
    }
}
