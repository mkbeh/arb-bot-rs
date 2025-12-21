use std::str::FromStr;

use async_trait::async_trait;
use solana_sdk::pubkey::Pubkey;
use tokio_util::sync::CancellationToken;

use crate::{
    config::{Config, Dex},
    libs::solana_rpc::{Event, GrpcClient, GrpcConfig},
    services::Exchange,
};

/// Core service for arbitrage operations.
pub struct ExchangeService {
    grpc_endpoint: String,
    x_token: Option<String>,
    exchanges: Vec<Dex>,
}

impl ExchangeService {
    pub fn from_config(config: &Config) -> anyhow::Result<Self> {
        Ok(Self {
            grpc_endpoint: config.solana_dex.grpc_endpoint.clone(),
            x_token: config.solana_dex.x_token.clone(),
            exchanges: config.solana_dex.exchanges.clone(),
        })
    }
}

#[async_trait]
impl Exchange for ExchangeService {
    /// Starts the arbitrage process.
    async fn start_arbitrage(&self, token: CancellationToken) -> anyhow::Result<()> {
        let program_ids = self
            .exchanges
            .iter()
            .map(|d| Pubkey::from_str(&d.program_id).unwrap())
            .collect::<Vec<_>>();

        let config = GrpcConfig {
            endpoint: self.grpc_endpoint.clone(),
            x_token: self.x_token.clone(),
            options: None,
            program_ids,
        };

        GrpcClient::new(config)
            .with_callback(|event: Event| Ok(()))
            .subscribe(token)
            .await?;

        Ok(())
    }
}
