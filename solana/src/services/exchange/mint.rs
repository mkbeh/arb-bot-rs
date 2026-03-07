use std::{sync::Arc, time::Duration};

use ahash::AHashSet;
use solana_sdk::pubkey::Pubkey;
use tokio_util::sync::CancellationToken;
use tools::misc::backoff::ExponentialBackoff;
use tracing::{error, warn};

use crate::{
    libs::solana_client::RpcClient,
    services::exchange::cache::{MINT_CACHE, PoolCache, get_market_state},
};

/// A background service responsible for keeping the global Mint account cache up to date.
///
/// The service periodically scans the pool cache for unique mints (tokens) and fetches
/// their latest account data (decimals, transfer fees, etc.) via RPC.
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
            chunk_size: 10,
            refresh_interval: Duration::from_secs(60),
        }
    }

    /// Starts the background synchronization loop.
    pub async fn start(&self, token: CancellationToken) -> anyhow::Result<()> {
        let mut interval = tokio::time::interval(self.refresh_interval);

        let mut backoff = ExponentialBackoff::new(
            Duration::from_secs(1),
            Duration::from_secs(60),
            Duration::from_secs(30),
        );

        loop {
            tokio::select! {
                _ = token.cancelled() => break,
                _ = interval.tick() => {
                    match self.refresh().await {
                        Ok(()) => {
                            backoff.reset();
                        }
                        Err(e) => {
                            let delay = backoff.next_delay();
                            error!("Failed to refresh mint cache: {e}. Retrying in {delay:?}...");

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

    /// Internal logic to perform a single synchronization cycle.
    async fn refresh(&self) -> anyhow::Result<()> {
        let mints: Vec<Pubkey> = {
            let cache = get_market_state().read();
            Self::collect_mints(&cache.pools)
        };

        if mints.is_empty() {
            return Ok(());
        }

        for chunk in mints.chunks(self.chunk_size) {
            let accounts = self.rpc.get_multiple_accounts(chunk).await?;
            let mut mint_cache = MINT_CACHE.write().await;

            for (pubkey, account_opt) in chunk.iter().zip(accounts.into_iter()) {
                if let Some(account) = account_opt {
                    mint_cache.update(*pubkey, account);
                } else {
                    warn!("Mint account not found for {}, removing from cache", pubkey);
                    mint_cache.data.remove(pubkey);
                }
            }
        }

        Ok(())
    }

    /// Iterates through all tracked pools and extracts unique mint addresses.
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
