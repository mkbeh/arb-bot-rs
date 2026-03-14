use std::{collections::hash_map::Entry, time::Instant};

use ahash::AHashMap;
use solana_sdk::pubkey::Pubkey;

use crate::services::exchange::cache::VAULT_CACHE_METRICS;

/// Represents a cached vault amount with metadata.
#[derive(Debug, Clone)]
pub struct CachedVault {
    /// The token amount held in the vault.
    pub amount: u64,
    /// Local timestamp when the cache was updated.
    pub updated_at: Instant,
}

/// Cache for vault token amounts.
pub struct VaultCache {
    /// Internal storage mapping vault addresses to their cached amounts.
    data: AHashMap<Pubkey, CachedVault>,
}

impl Default for VaultCache {
    fn default() -> Self {
        Self::new()
    }
}

impl VaultCache {
    #[must_use]
    pub fn new() -> Self {
        Self {
            data: AHashMap::with_capacity(1024),
        }
    }

    /// Updates or inserts a vault amount into the cache.
    pub fn update(&mut self, key: Pubkey, amount: u64) {
        match self.data.entry(key) {
            Entry::Occupied(mut entry) => {
                let cached = entry.get_mut();
                cached.amount = amount;
                cached.updated_at = Instant::now();
            }
            Entry::Vacant(entry) => {
                entry.insert(CachedVault {
                    amount,
                    updated_at: Instant::now(),
                });
            }
        }

        VAULT_CACHE_METRICS.set_cache_size(self.data.len())
    }

    /// Retrieves the cached vault amount.
    #[inline]
    #[must_use]
    pub fn get(&self, key: &Pubkey) -> Option<u64> {
        self.data.get(key).map(|cached| cached.amount)
    }
}
