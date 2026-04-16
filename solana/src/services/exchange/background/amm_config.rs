use std::{sync::Arc, time::Duration};

use anyhow::anyhow;
use async_trait::async_trait;
use bytemuck::Pod;
use solana_client::{
    rpc_config::{
        CommitmentConfig, RpcAccountInfoConfig, RpcProgramAccountsConfig, UiAccountEncoding,
    },
    rpc_filter::{Memcmp, MemcmpEncodedBytes, RpcFilterType},
};
use tracing::log::warn;

use super::BackgroundService;
use crate::{
    libs::solana_client::{
        RpcClient,
        pool::AmmConfigEntry,
        protocols::{raydium_clmm, raydium_cpmm},
        registry::ProtocolEntity,
    },
    services::exchange::cache::get_amm_config_cache,
};

pub struct AmmConfigService {
    rpc: Arc<RpcClient>,
    refresh_interval: Duration,
}

impl AmmConfigService {
    #[must_use]
    pub fn new(rpc: Arc<RpcClient>) -> Self {
        Self {
            rpc,
            refresh_interval: Duration::from_secs(60),
        }
    }

    async fn fetch_and_cache<T>(&self) -> anyhow::Result<()>
    where
        T: ProtocolEntity + AmmConfigEntry + Pod + Copy + std::fmt::Debug,
    {
        let config = RpcProgramAccountsConfig {
            filters: Some(vec![
                RpcFilterType::DataSize(T::DATA_SIZE as u64),
                RpcFilterType::Memcmp(Memcmp::new(
                    0,
                    MemcmpEncodedBytes::Base58(bs58::encode(T::DISCRIMINATOR).into_string()),
                )),
            ]),
            account_config: RpcAccountInfoConfig {
                encoding: Some(UiAccountEncoding::Base64),
                commitment: Some(CommitmentConfig::confirmed()),
                ..Default::default()
            },
            ..Default::default()
        };

        let accounts = self
            .rpc
            .get_program_accounts_with_config(&T::PROGRAM_ID, config)
            .await?;

        let mut cache = get_amm_config_cache().write();

        for (pubkey, ui_account) in accounts {
            let data = ui_account
                .data
                .decode()
                .ok_or_else(|| anyhow!("Failed to decode account data for {pubkey}"))?;

            if let Some(config) = T::deserialize(&data) {
                cache.insert(pubkey, config)
            } else {
                warn!("Failed to deserialize AmmConfig for {pubkey}")
            }
        }

        Ok(())
    }
}

#[async_trait]
impl BackgroundService for AmmConfigService {
    fn execute_interval(&self) -> Duration {
        self.refresh_interval
    }

    async fn execute(&self) -> anyhow::Result<()> {
        self.fetch_and_cache::<raydium_clmm::AmmConfig>().await?;
        self.fetch_and_cache::<raydium_cpmm::AmmConfig>().await?;
        Ok(())
    }
}
