use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

#[async_trait]
pub trait ArbitrageService: Send + Sync {
    async fn start(&self, token: CancellationToken) -> anyhow::Result<()>;
}

#[async_trait]
pub trait Exchange: ArbitrageService {}

#[async_trait]
pub trait Sender: ArbitrageService {}
