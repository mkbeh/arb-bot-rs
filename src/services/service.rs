use async_trait::async_trait;

#[async_trait]
pub trait ExchangeService: Send + Sync {
    async fn start_arbitrage(&self) -> anyhow::Result<()>;
}
