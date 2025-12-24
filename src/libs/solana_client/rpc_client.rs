use anyhow::Context;
use solana_client::{
    nonblocking::rpc_client::RpcClient as SolanaRpcClient, rpc_config::CommitmentConfig,
};
use solana_sdk::hash::Hash;

/// Configuration for the Solana RPC client.
#[derive(Default)]
pub struct RpcConfig {
    /// The Solana RPC endpoint URL.
    pub url: String,
}

/// Wrapper for Solana RPC client.
pub struct RpcClient {
    /// Internal RPC client instance.
    client: SolanaRpcClient,
}

impl RpcClient {
    /// Creates a new `RpcClient` from the provided configuration.
    pub fn from_config(config: RpcConfig) -> Self {
        let client = SolanaRpcClient::new(config.url);
        Self { client }
    }

    /// Gets the latest blockhash and its validity height for transaction signing.
    pub async fn get_recent_blockhash(&self) -> anyhow::Result<(Hash, u64)> {
        let (blockhash, last_valid_height) = self
            .client
            .get_latest_blockhash_with_commitment(CommitmentConfig::processed())
            .await
            .context("Failed to get recent blockhash from RPC")?;
        Ok((blockhash, last_valid_height))
    }
}
