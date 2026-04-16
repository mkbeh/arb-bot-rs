use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use solana_sdk::pubkey::Pubkey;
use tracing::log::warn;

use super::BackgroundService;
use crate::{
    libs::solana_client::RpcClient,
    services::exchange::cache::{get_market_state, get_mint_cache},
};

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
            refresh_interval: Duration::from_secs(30),
        }
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
            cache.pools().get_pool_mints()
        };

        if mints.is_empty() {
            return Ok(());
        }

        for chunk in mints.chunks(self.chunk_size) {
            let response = self.rpc.get_multiple_accounts(chunk).await?;
            let mut mint_cache = get_mint_cache().write();

            for (pubkey, account_opt) in chunk.iter().zip(response.value) {
                if let Some(account) = account_opt {
                    mint_cache.update(*pubkey, account);
                } else {
                    warn!("Mint account not found for {pubkey}, removing from cache");
                    mint_cache.remove(pubkey);
                }
            }
        }

        Ok(())
    }
}
