use std::{collections::hash_map::Entry, sync::LazyLock, time::Instant};

use ahash::AHashMap;
use solana_sdk::{account::Account, pubkey::Pubkey};
use tokio::sync::RwLock;

use crate::services::exchange::cache::MINT_CACHE_METRICS;

/// Global thread-safe instance of the `MintCache`.
pub static MINT_CACHE: LazyLock<RwLock<MintCache>> =
    LazyLock::new(|| RwLock::new(MintCache::default()));

/// Represents a cached Solana account with metadata.
#[derive(Debug, Clone)]
pub struct CachedAccount {
    /// The actual account data retrieved from the network.
    pub account: Account,
    /// Local timestamp when the cache was updated.
    pub updated_at: Instant,
}

/// Cache for Mint accounts.
pub struct MintCache {
    /// Internal storage mapping account addresses to their cached state.
    data: AHashMap<Pubkey, CachedAccount>,
}

impl Default for MintCache {
    fn default() -> Self {
        Self::new()
    }
}

impl MintCache {
    #[must_use]
    pub fn new() -> Self {
        Self {
            data: AHashMap::with_capacity(1024),
        }
    }

    /// Updates or inserts an account into the cache.
    pub fn update(&mut self, key: Pubkey, account: Account) {
        match self.data.entry(key) {
            Entry::Occupied(mut entry) => {
                // Perform an in-place update of the existing cached entry.
                let cached = entry.get_mut();
                cached.account = account;
                cached.updated_at = Instant::now();
            }
            Entry::Vacant(entry) => {
                // Insert a new entry if the key doesn't exist in the cache.
                entry.insert(CachedAccount {
                    account,
                    updated_at: Instant::now(),
                });
            }
        }

        MINT_CACHE_METRICS.set_cache_size(self.data.len());
    }

    /// Retrieves a reference to the cached account.
    #[inline]
    #[must_use]
    pub fn get(&self, key: &Pubkey) -> Option<&Account> {
        self.data.get(key).map(|cached| &cached.account)
    }

    /// Removes an account from the cache by its public key.
    pub fn remove(&mut self, key: &Pubkey) {
        self.data.remove(key);
    }
}
