use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

use crate::{config::Config, services::Sender};

/// Service for sending orders from arbitrage chains.
pub struct SenderService {
    // todo
}

impl SenderService {
    pub fn from_config(config: &Config) -> anyhow::Result<Self> {
        todo!()
    }
}

#[async_trait]
impl Sender for SenderService {
    async fn send_orders(&self, token: CancellationToken) -> anyhow::Result<()> {
        todo!()
    }
}
