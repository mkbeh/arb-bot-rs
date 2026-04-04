use anyhow::Context;
use solana_client::{
    nonblocking::rpc_client::RpcClient as SolanaRpcClient,
    rpc_config::{CommitmentConfig, RpcProgramAccountsConfig},
    rpc_response::UiAccount,
};
use solana_sdk::{account::Account, pubkey::Pubkey};

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
    #[must_use]
    pub fn from_config(config: RpcConfig) -> Self {
        let client = SolanaRpcClient::new(config.url);
        Self { client }
    }

    /// Returns the account information for a list of pubkeys.
    pub async fn get_multiple_accounts(
        &self,
        pubkeys: &[Pubkey],
    ) -> anyhow::Result<Vec<Option<Account>>> {
        let accounts = self
            .client
            .get_multiple_accounts_with_commitment(pubkeys, CommitmentConfig::confirmed())
            .await
            .context("Failed to get multiple accounts from RPC")?;
        Ok(accounts.value)
    }

    /// Returns all accounts owned by the provided program pubkey.
    pub async fn get_program_accounts_with_config(
        &self,
        pubkey: &Pubkey,
        config: RpcProgramAccountsConfig,
    ) -> anyhow::Result<Vec<(Pubkey, UiAccount)>> {
        self.client
            .get_program_ui_accounts_with_config(pubkey, config)
            .await
            .context("Failed to get program ui accounts")
    }
}
