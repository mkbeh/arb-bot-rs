use std::{sync::Arc, time::Duration};

use anyhow::Context;
use async_trait::async_trait;
use solana_client::{
    rpc_config::{
        CommitmentConfig, RpcAccountInfoConfig, RpcProgramAccountsConfig, UiAccountEncoding,
    },
    rpc_filter::{Memcmp, RpcFilterType},
};
use solana_sdk::{account::Account, pubkey::Pubkey};
use tracing::{error, log::warn};

use super::BackgroundService;
use crate::{
    libs::solana_client::{
        RpcClient,
        models::*,
        protocols::{meteora_dlmm::*, orca::*, raydium_clmm::TickArrayState},
        registry::*,
        utils::get_timestamp_ms,
    },
    services::exchange::cache::{PoolSyncStatus, get_market_state, get_pool_sync_cache},
};

/// Background service responsible for the initial warm-up of pool data.
///
/// Runs frequently (every few seconds) until all pending pools are synced.
pub struct LazySyncService {
    syncer: Syncer,
    pools_batch_size: usize,
    refresh_interval: Duration,
}

impl LazySyncService {
    #[must_use]
    pub fn new(rpc: Arc<RpcClient>) -> Self {
        Self {
            syncer: Syncer {
                rpc,
                chunk_size: 100,
                request_timeout: Duration::from_millis(200),
            },
            pools_batch_size: 10,
            refresh_interval: Duration::from_secs(5),
        }
    }
}

#[async_trait]
impl BackgroundService for LazySyncService {
    fn execute_interval(&self) -> Duration {
        self.refresh_interval
    }

    async fn execute(&self) -> anyhow::Result<()> {
        let pending = get_pool_sync_cache()
            .read()
            .get_pending_pools(self.pools_batch_size);

        if pending.is_empty() {
            return Ok(());
        }

        let mut bitmap_pubkeys: Vec<Pubkey> = Vec::new();
        let mut liquidity_pools: Vec<(Pubkey, ProtocolKind)> = Vec::new();

        for (pool_id, status) in &pending {
            let PoolSyncStatus::Pending {
                needs_liquidity,
                needs_bitmap,
            } = status
            else {
                continue;
            };

            let Some(protocol) = get_market_state()
                .read()
                .pools()
                .get_pool(pool_id)
                .map(|p| p.protocol())
            else {
                continue;
            };

            if *needs_bitmap && let Some(bitmap_pda) = protocol.bitmap_pda(pool_id) {
                bitmap_pubkeys.push(bitmap_pda);
            }

            if *needs_liquidity {
                liquidity_pools.push((*pool_id, protocol));
            }
        }

        let mut events = Vec::new();

        if !bitmap_pubkeys.is_empty() {
            self.syncer
                .fetch_bitmaps(&bitmap_pubkeys, &mut events)
                .await?;
        }

        if !liquidity_pools.is_empty() {
            self.syncer
                .fetch_liquidity(&liquidity_pools, &mut events)
                .await?;
        }

        if !events.is_empty() {
            get_market_state().write().update_events(events);
        }

        let ts = get_timestamp_ms();
        let mut sync_cache = get_pool_sync_cache().write();

        for (pool_id, _) in &liquidity_pools {
            sync_cache.mark_ready(*pool_id, ts)
        }

        Ok(())
    }
}

/// Background service responsible for periodic refresh of data
/// for pools that are already in `Ready` state.
pub struct ReSyncService {
    syncer: Syncer,
    pools_batch_size: usize,
    pools_max_age_ms: u64,
    refresh_interval: Duration,
}

impl ReSyncService {
    #[must_use]
    pub fn new(rpc: Arc<RpcClient>) -> Self {
        Self {
            syncer: Syncer {
                rpc,
                chunk_size: 100,
                request_timeout: Duration::from_millis(200),
            },
            pools_batch_size: 10,
            pools_max_age_ms: 3_600_000, // 1h
            refresh_interval: Duration::from_secs(5),
        }
    }
}

#[async_trait]
impl BackgroundService for ReSyncService {
    fn execute_interval(&self) -> Duration {
        self.refresh_interval
    }

    async fn execute(&self) -> anyhow::Result<()> {
        let ready = get_pool_sync_cache()
            .read()
            .get_ready_pools(self.pools_max_age_ms, self.pools_batch_size);

        if ready.is_empty() {
            return Ok(());
        }

        let mut liquidity_pools: Vec<(Pubkey, ProtocolKind)> = Vec::new();

        for pool_id in &ready {
            let Some(protocol) = get_market_state()
                .read()
                .pools()
                .get_pool(pool_id)
                .map(|p| p.protocol())
            else {
                continue;
            };

            if !LiquidityFetchConfig::build_protocol_configs(*pool_id, protocol).is_empty() {
                liquidity_pools.push((*pool_id, protocol));
            }

            let mut events = Vec::new();

            if !liquidity_pools.is_empty() {
                self.syncer
                    .fetch_liquidity(&liquidity_pools, &mut events)
                    .await?;
            }

            if !events.is_empty() {
                get_market_state().write().update_events(events);
            }

            let ts = get_timestamp_ms();
            let mut sync_cache = get_pool_sync_cache().write();

            for (pool_id, _) in &liquidity_pools {
                sync_cache.mark_ready(*pool_id, ts)
            }
        }

        Ok(())
    }
}

struct Syncer {
    rpc: Arc<RpcClient>,
    chunk_size: usize,
    request_timeout: Duration,
}

impl Syncer {
    async fn fetch_bitmaps(
        &self,
        bitmap_pubkeys: &[Pubkey],
        events: &mut Vec<Event>,
    ) -> anyhow::Result<()> {
        for chunk in bitmap_pubkeys.chunks(self.chunk_size) {
            let response = self.rpc.get_multiple_accounts(chunk).await.map_err(|e| {
                warn!("Failed to fetch bitmap accounts: {e:#}");
                e
            })?;

            for (pubkey, account_opt) in chunk.iter().zip(response.value) {
                let Some(account) = account_opt else { continue };
                let (_item, pool_state) =
                    Self::parse_account_payload(&account.owner, pubkey, &account.data)
                        .with_context(|| format!("PARSING ERROR: account: {pubkey}"))?;

                events.push(Event::Program(Box::new(ProgramEvent {
                    slot: response.context.slot,
                    is_startup: false,
                    pubkey: *pubkey,
                    lamports: account.lamports,
                    owner: account.owner,
                    executable: account.executable,
                    rent_epoch: account.rent_epoch,
                    write_version: None,
                    txn_signature: None,
                    pool_state,
                })));
            }
        }
        Ok(())
    }

    async fn fetch_liquidity(
        &self,
        liquidity_pools: &[(Pubkey, ProtocolKind)],
        events: &mut Vec<Event>,
    ) -> anyhow::Result<()> {
        let slot = self.rpc.get_slot().await?;

        for (pool_id, protocol) in liquidity_pools {
            let configs = LiquidityFetchConfig::build_protocol_configs(*pool_id, *protocol);

            for config in configs {
                let accounts = Self::fetch_liquidity_accounts(&self.rpc, config)
                    .await
                    .map_err(|e| {
                        warn!("Failed to fetch liquidity for pool {pool_id}: {e:#}");
                        e
                    })?;

                for (pubkey, account) in accounts {
                    let (_item, pool_state) =
                        Self::parse_account_payload(&account.owner, &pubkey, &account.data)
                            .with_context(|| format!("PARSING ERROR: account: {pubkey}"))?;

                    events.push(Event::Program(Box::new(ProgramEvent {
                        slot,
                        is_startup: false,
                        pubkey,
                        lamports: account.lamports,
                        owner: account.owner,
                        executable: account.executable,
                        rent_epoch: account.rent_epoch,
                        write_version: None,
                        txn_signature: None,
                        pool_state,
                    })));
                }

                tokio::time::sleep(self.request_timeout).await;
            }
        }

        Ok(())
    }

    async fn fetch_liquidity_accounts(
        rpc: &RpcClient,
        config: LiquidityFetchConfig,
    ) -> anyhow::Result<Vec<(Pubkey, Account)>> {
        let mut filters = vec![RpcFilterType::Memcmp(Memcmp::new_raw_bytes(
            config.pool_id_offset,
            config.pool_id.to_bytes().to_vec(),
        ))];

        if config.data_size > 0 {
            filters.push(RpcFilterType::DataSize(config.data_size as u64));
        }

        if !config.discriminator.is_empty() {
            filters.push(RpcFilterType::Memcmp(Memcmp::new_raw_bytes(
                0,
                config.discriminator.to_vec(),
            )));
        }

        let rpc_config = RpcProgramAccountsConfig {
            filters: Some(filters),
            account_config: RpcAccountInfoConfig {
                encoding: Some(UiAccountEncoding::Base64),
                commitment: Some(CommitmentConfig::confirmed()),
                ..Default::default()
            },
            ..Default::default()
        };

        let accounts = rpc
            .get_program_accounts_with_config(&config.program_id, rpc_config)
            .await?;

        Ok(accounts
            .into_iter()
            .filter_map(|(pubkey, ui_account)| {
                let account = ui_account.decode::<Account>()?;
                Some((pubkey, account))
            })
            .collect())
    }

    fn parse_account_payload<'a>(
        program_id: &Pubkey,
        pubkey: &Pubkey,
        payload: &[u8],
    ) -> Option<(&'a RegistryItem, PoolState)> {
        let item = PROTOCOL_REGISTRY
            .get_account_item(program_id, payload.len(), payload)
            .or_else(|| {
                warn!(
                    "No registered parser found for program {program_id} with data size {}",
                    payload.len()
                );
                None
            })?;

        let ProtocolParser::Program(parser_fn) = &item.parser else {
            error!("Registry integrity error: Expected Account parser for {program_id}");
            return None;
        };

        let pool_state = parser_fn(payload).or_else(|| {
            error!(
                "[{}] Failed to parse account: {}. Data size: {}",
                item.name,
                pubkey,
                payload.len()
            );
            None
        })?;

        Some((item, pool_state))
    }
}

#[derive(Debug, Clone)]
pub struct LiquidityFetchConfig {
    pub program_id: Pubkey,
    pub pool_id: Pubkey,
    pub pool_id_offset: usize,
    pub data_size: usize,
    pub discriminator: [u8; 8],
}

impl LiquidityFetchConfig {
    fn build_protocol_configs(pool_id: Pubkey, protocol: ProtocolKind) -> Vec<Self> {
        match protocol {
            ProtocolKind::MeteoraDlmm => {
                vec![Self {
                    program_id: BinArray::PROGRAM_ID,
                    pool_id_offset: BinArray::POOL_ID_OFFSET,
                    data_size: BinArray::DATA_SIZE,
                    discriminator: BinArray::DISCRIMINATOR.try_into().unwrap(),
                    pool_id,
                }]
            }
            ProtocolKind::RaydiumClmm => {
                vec![Self {
                    program_id: TickArrayState::PROGRAM_ID,
                    pool_id_offset: TickArrayState::POOL_ID_OFFSET,
                    data_size: TickArrayState::DATA_SIZE,
                    discriminator: TickArrayState::DISCRIMINATOR.try_into().unwrap(),
                    pool_id,
                }]
            }
            ProtocolKind::Orca => {
                vec![
                    Self {
                        program_id: FixedTickArray::PROGRAM_ID,
                        pool_id_offset: FixedTickArray::POOL_ID_OFFSET,
                        data_size: FixedTickArray::DATA_SIZE,
                        discriminator: FixedTickArray::DISCRIMINATOR.try_into().unwrap(),
                        pool_id,
                    },
                    Self {
                        program_id: DynamicTickArray::PROGRAM_ID,
                        pool_id_offset: DynamicTickArray::POOL_ID_OFFSET,
                        data_size: DynamicTickArray::DATA_SIZE,
                        discriminator: DynamicTickArray::DISCRIMINATOR.try_into().unwrap(),
                        pool_id,
                    },
                ]
            }
            _ => vec![],
        }
    }
}
