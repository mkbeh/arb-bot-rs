use std::{collections::hash_map::Entry, sync::LazyLock};

use ahash::AHashMap;
use parking_lot::RwLock;
use solana_sdk::pubkey::Pubkey;

use crate::{
    libs::solana_client::metrics::ProtocolKind, services::exchange::cache::POOL_SYNC_CACHE_METRICS,
};

static POOL_SYNC_CACHE: LazyLock<RwLock<PoolSyncCache>> =
    LazyLock::new(|| RwLock::new(PoolSyncCache::default()));

#[must_use]
pub fn get_pool_sync_cache() -> &'static RwLock<PoolSyncCache> {
    &POOL_SYNC_CACHE
}

pub enum PoolSyncStatus {
    NotRequired,
    Pending {
        needs_liquidity: bool,
        needs_bitmap: bool,
    },
    Ready {
        synced_at: u64,
    },
}

impl PoolSyncStatus {
    #[must_use]
    pub fn is_ready(&self) -> bool {
        matches!(self, Self::Ready { .. } | Self::NotRequired)
    }

    #[must_use]
    pub fn is_pending(&self) -> bool {
        matches!(self, Self::Pending { .. })
    }

    pub fn mark_ready(&mut self, ts: u64) {
        *self = PoolSyncStatus::Ready { synced_at: ts }
    }
}

#[must_use]
pub fn get_sync_status_by_protocol(kind: &ProtocolKind) -> PoolSyncStatus {
    let needs_liquidity = matches!(
        kind,
        ProtocolKind::Orca | ProtocolKind::RaydiumClmm | ProtocolKind::MeteoraDlmm
    );
    let needs_bitmap = matches!(kind, ProtocolKind::RaydiumClmm | ProtocolKind::MeteoraDlmm);

    if !needs_liquidity && !needs_bitmap {
        return PoolSyncStatus::NotRequired;
    }

    PoolSyncStatus::Pending {
        needs_liquidity,
        needs_bitmap,
    }
}

#[derive(Default)]
pub struct PoolSyncCache {
    statuses: AHashMap<Pubkey, PoolSyncStatus>,
}

impl PoolSyncCache {
    pub fn init(&mut self, pool_id: Pubkey, status: PoolSyncStatus) {
        if let Entry::Vacant(e) = self.statuses.entry(pool_id) {
            if status.is_pending() {
                POOL_SYNC_CACHE_METRICS.set_pending();
            }
            e.insert(status);
        }
    }

    pub fn mark_ready(&mut self, pool_id: Pubkey, ts: u64) {
        if let Some(status) = self.statuses.get_mut(&pool_id) {
            if status.is_pending() {
                POOL_SYNC_CACHE_METRICS.set_ready();
            }
            status.mark_ready(ts);
        }
    }

    #[must_use]
    pub fn is_ready(&self, pool_id: &Pubkey) -> bool {
        self.statuses
            .get(pool_id)
            .map(|s| s.is_ready())
            .unwrap_or(false)
    }

    #[must_use]
    pub fn get_pending_pools(&self) -> Vec<(Pubkey, &PoolSyncStatus)> {
        self.statuses
            .iter()
            .filter(|(_, s)| matches!(s, PoolSyncStatus::Pending { .. }))
            .map(|(pool_id, status)| (*pool_id, status))
            .collect()
    }
}
