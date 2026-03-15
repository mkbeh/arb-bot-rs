use std::{sync::Arc, time::Duration};

use ahash::AHashSet;
use anyhow::anyhow;
use async_trait::async_trait;
use bytemuck::Pod;
use solana_client::{
    rpc_config::{
        CommitmentConfig, RpcAccountInfoConfig, RpcProgramAccountsConfig, UiAccountEncoding,
    },
    rpc_filter::{Memcmp, MemcmpEncodedBytes, RpcFilterType},
};
use solana_sdk::pubkey::Pubkey;
use tokio_util::sync::CancellationToken;
use tools::misc::backoff::ExponentialBackoff;
use tracing::{error, warn};

use crate::{
    libs::solana_client::{
        RpcClient,
        dex::{raydium_clmm, raydium_cpmm},
        pool::AmmConfigEntry,
        registry::DexEntity,
    },
    services::exchange::cache::*,
};

// ------------------------------------------------------------------ //
//  BackgroundService trait                                           //
// ------------------------------------------------------------------ //

#[async_trait]
pub trait BackgroundService {
    fn execute_interval(&self) -> Duration;
    async fn execute(&self) -> anyhow::Result<()>;

    async fn start(&self, token: CancellationToken) -> anyhow::Result<()> {
        let mut interval = tokio::time::interval(self.execute_interval());

        let mut backoff = ExponentialBackoff::new(
            Duration::from_secs(1),
            Duration::from_secs(60),
            Duration::from_secs(30),
        );

        loop {
            tokio::select! {
                _ = token.cancelled() => break,
                _ = interval.tick() => {
                    match self.execute().await {
                        Ok(()) => {
                            backoff.reset();
                        }
                        Err(e) => {
                            let delay = backoff.next_delay();
                            error!(
                                "[{}] Failed to execute: {e}. Retrying in {delay:?}...",
                                std::any::type_name::<Self>()
                            );

                            tokio::select! {
                                _ = token.cancelled() => break,
                                _ = tokio::time::sleep(delay) => {}
                            }

                            interval.reset();
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

// ------------------------------------------------------------------ //
//  MintService                                                       //
// ------------------------------------------------------------------ //

pub struct MintService {
    rpc: Arc<RpcClient>,
    chunk_size: usize,
    refresh_interval: Duration,
}

impl MintService {
    #[must_use]
    pub fn new(rpc: Arc<RpcClient>) -> Self {
        Self {
            rpc,
            chunk_size: 100,
            refresh_interval: Duration::from_secs(60),
        }
    }

    fn collect_mints(cache: &PoolCache) -> Vec<Pubkey> {
        let mut mints = AHashSet::with_capacity(cache.len() * 2);

        for pool in cache.values() {
            let (mint_a, mint_b) = pool.get_mints();
            mints.insert(mint_a);
            mints.insert(mint_b);
        }

        mints.into_iter().collect()
    }
}

#[async_trait]
impl BackgroundService for MintService {
    fn execute_interval(&self) -> Duration {
        self.refresh_interval
    }

    async fn execute(&self) -> anyhow::Result<()> {
        let mints: Vec<Pubkey> = {
            let cache = get_market_state().read();
            Self::collect_mints(&cache.pools)
        };

        if mints.is_empty() {
            return Ok(());
        }

        for chunk in mints.chunks(self.chunk_size) {
            let accounts = self.rpc.get_multiple_accounts(chunk).await?;
            let mut mint_cache = get_mint_cache().write();

            for (pubkey, account_opt) in chunk.iter().zip(accounts) {
                if let Some(account) = account_opt {
                    mint_cache.update(*pubkey, account);
                } else {
                    warn!("Mint account not found for {}, removing from cache", pubkey);
                    mint_cache.remove(pubkey);
                }
            }
        }

        Ok(())
    }
}

// ------------------------------------------------------------------ //
//  AmmConfigService                                                  //
// ------------------------------------------------------------------ //

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
        T: DexEntity + AmmConfigEntry + Pod + Copy + std::fmt::Debug,
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
