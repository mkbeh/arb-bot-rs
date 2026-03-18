use std::{sync::Arc, time::Duration};

use solana_sdk::{account::Account, pubkey::Pubkey};
use tokio::{sync::mpsc, task::JoinSet, time::timeout};
use tracing::warn;

use crate::{
    libs::solana_client::{
        RpcClient, SolanaStream, callback::BatchEventCallbackWrapper, models::Event,
    },
    services::exchange::{cache::get_market_state, compute::PoolUpdate},
};

/// Processes incoming on-chain account events and updates the global market state.
pub struct MarketService {
    rpc: Arc<RpcClient>,
    vault_rpc_timeout: Duration,
    vault_rpc_chunk_size: usize,
    compute_tx: mpsc::Sender<PoolUpdate>,
}

impl MarketService {
    /// Creates a new `MarketService` instance.
    #[must_use]
    pub fn new(rpc: Arc<RpcClient>, compute_tx: mpsc::Sender<PoolUpdate>) -> Self {
        Self {
            rpc,
            vault_rpc_timeout: Duration::from_millis(500),
            vault_rpc_chunk_size: 100,
            compute_tx,
        }
    }

    /// Attaches the market event handler to the given stream.
    pub fn bind_to(self: Arc<Self>, stream: &mut Box<dyn SolanaStream>) {
        let wrapper = BatchEventCallbackWrapper::new(move |events: Vec<Event>| {
            let service = self.clone();
            async move { service.handle_events(events).await }
        });
        stream.set_callback(wrapper)
    }

    /// Returns a batch event handler that processes account updates.
    async fn handle_events(&self, events: Vec<Event>) -> anyhow::Result<()> {
        let result = {
            let mut market = get_market_state().write();
            market.update_states(events)
        };

        if !result.vaults.is_empty() {
            let vault_list: Vec<Pubkey> = result.vaults.into_iter().collect();
            self.refresh_vault_balances(&vault_list).await?;
        }

        if !result.changed_pools.is_empty() {
            let update = PoolUpdate {
                changed_pools: result.changed_pools.into_iter().collect(),
            };

            if let Err(e) = self.compute_tx.send(update).await {
                warn!("Failed to send pool update to compute service: {e}");
            }
        }

        Ok(())
    }

    async fn refresh_vault_balances(&self, vault_pubkeys: &[Pubkey]) -> anyhow::Result<()> {
        if vault_pubkeys.is_empty() {
            return Ok(());
        }

        let mut set = JoinSet::new();
        for chunk in vault_pubkeys.chunks(self.vault_rpc_chunk_size) {
            let rpc = Arc::clone(&self.rpc);
            let chunk_vec = chunk.to_vec();
            let vault_rpc_timeout = self.vault_rpc_timeout;

            set.spawn(async move {
                let accounts = timeout(vault_rpc_timeout, rpc.get_multiple_accounts(&chunk_vec))
                    .await
                    .map_err(|_| {
                        anyhow::anyhow!("RPC timeout for chunk of {}", chunk_vec.len())
                    })??;
                Ok::<(Vec<Pubkey>, Vec<Option<Account>>), anyhow::Error>((chunk_vec, accounts))
            });
        }

        let chunks_count = vault_pubkeys.len().div_ceil(self.vault_rpc_chunk_size);
        let mut results = Vec::with_capacity(chunks_count);

        while let Some(res) = set.join_next().await {
            let (pubkeys, accounts) = res.map_err(|e| anyhow::anyhow!("Task panicked: {e}"))??;
            results.push((pubkeys, accounts));
        }

        let mut market = get_market_state().write();
        for (pubkeys, accounts) in results {
            market.update_vaults(&pubkeys, accounts);
        }

        Ok(())
    }
}
