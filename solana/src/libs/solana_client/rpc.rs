use anyhow::Context;
use solana_client::{
    nonblocking::rpc_client::RpcClient as SolanaRpcClient,
    rpc_config::{CommitmentConfig, RpcProgramAccountsConfig},
    rpc_response::{Response, UiAccount},
};
use solana_sdk::{account::Account, clock::Slot, pubkey::Pubkey};

use crate::libs::solana_client::metrics::MeterSender;

#[derive(Default)]
pub struct RpcConfig {
    pub url: String,
}

pub struct RpcClient {
    inner: SolanaRpcClient,
}

impl RpcClient {
    #[must_use]
    pub fn from_config(config: RpcConfig) -> Self {
        let sender = MeterSender::new(config.url);
        let client = SolanaRpcClient::new_sender(sender, Default::default());
        Self { inner: client }
    }

    pub async fn get_slot(&self) -> anyhow::Result<Slot> {
        self.inner
            .get_slot_with_commitment(CommitmentConfig::confirmed())
            .await
            .context("Failed to get slot")
    }

    pub async fn get_multiple_accounts(
        &self,
        pubkeys: &[Pubkey],
    ) -> anyhow::Result<Response<Vec<Option<Account>>>> {
        self.inner
            .get_multiple_accounts_with_commitment(pubkeys, CommitmentConfig::confirmed())
            .await
            .context("Failed to get multiple accounts")
    }

    pub async fn get_program_accounts_with_config(
        &self,
        pubkey: &Pubkey,
        config: RpcProgramAccountsConfig,
    ) -> anyhow::Result<Vec<(Pubkey, UiAccount)>> {
        self.inner
            .get_program_ui_accounts_with_config(pubkey, config)
            .await
            .context("Failed to get program ui accounts")
    }
}
