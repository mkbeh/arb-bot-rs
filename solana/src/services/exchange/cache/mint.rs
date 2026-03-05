use std::{
    collections::hash_map::Entry,
    sync::{Arc, LazyLock},
    time::Instant,
};

use ahash::AHashMap;
use solana_sdk::{account::Account, pubkey::Pubkey};
use tokio::sync::RwLock;

use crate::services::exchange::cache::MINT_CACHE_METRICS;

/// Global thread-safe instance of the `MintCache`.
pub static MINT_CACHE: LazyLock<Arc<RwLock<MintCache>>> =
    LazyLock::new(|| Arc::new(RwLock::new(MintCache::default())));

/// Represents a cached Solana account with metadata.
#[derive(Debug, Clone)]
pub struct CachedAccount {
    /// The actual account data retrieved from the network.
    pub account: Account,
    /// The slot (block height) at which this account state was captured.
    pub slot: u64,
    /// Local timestamp when the cache was updated.
    pub updated_at: Instant,
}

/// Cache for Mint accounts.
pub struct MintCache {
    /// Internal storage mapping account addresses to their cached state.
    pub data: AHashMap<Pubkey, CachedAccount>,
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
    pub fn update(&mut self, key: Pubkey, account: Account, slot: u64) {
        match self.data.entry(key) {
            Entry::Occupied(mut entry) => {
                // Ignore the update if the incoming data is from an older or same slot.
                if slot <= entry.get().slot {
                    return;
                }
                // Perform an in-place update of the existing cached entry.
                let cached = entry.get_mut();
                cached.account = account;
                cached.slot = slot;
                cached.updated_at = Instant::now();
            }
            Entry::Vacant(entry) => {
                // Insert a new entry if the key doesn't exist in the cache.
                entry.insert(CachedAccount {
                    account,
                    slot,
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
}
