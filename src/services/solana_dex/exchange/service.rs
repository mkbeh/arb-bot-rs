use std::sync::Arc;

use anyhow::Context;
use async_trait::async_trait;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;
use tracing::error;

use crate::{
    config::Config,
    services::{Exchange, solana_dex::exchange::tx_stream::TxStream},
};

/// Core service for arbitrage operations.
pub struct ExchangeService {
    /// Stream for transaction events.
    tx_stream: Arc<TxStream>,
}

impl ExchangeService {
    /// Creates a new `ExchangeService` from configuration.
    pub fn from_config(config: &Config) -> anyhow::Result<Self> {
        let solana_settings = &config.solana_dex;
        let tx_stream = TxStream::new(
            solana_settings.grpc_endpoint.clone(),
            solana_settings.x_token.clone(),
            solana_settings.get_dex_programs(),
        );

        Ok(Self {
            tx_stream: Arc::new(tx_stream),
        })
    }
}

#[async_trait]
impl Exchange for ExchangeService {
    /// Starts the arbitrage process.
    async fn start_arbitrage(&self, token: CancellationToken) -> anyhow::Result<()> {
        let mut tasks_set = JoinSet::new();

        tasks_set.spawn({
            let tx_stream = self.tx_stream.clone();
            let token = token.clone();

            async move { tx_stream.start(token).await }
        });

        let result = tasks_set.join_next().await.context("Failed to join task")?;

        match result {
            Ok(Err(e)) => error!("task failed: {e}"),
            Err(e) => error!("join error: {e}"),
            _ => {}
        }

        Ok(())
    }
}
