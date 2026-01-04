use async_trait::async_trait;
use engine::{Sender, service::traits::ArbitrageService};
use tokio_util::sync::CancellationToken;

use crate::Config;

pub struct SenderService {
    //
}

impl SenderService {
    pub async fn from_config(config: &Config) -> anyhow::Result<Self> {
        Ok(Self {})
    }
}

impl Sender for SenderService {}

#[async_trait]
impl ArbitrageService for SenderService {
    async fn start(&self, token: CancellationToken) -> anyhow::Result<()> {
        Ok(())
    }
}
