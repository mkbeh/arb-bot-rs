use std::sync::LazyLock;

use ahash::AHashMap;
use parking_lot::RwLock;
use solana_sdk::pubkey::Pubkey;

use crate::{
    libs::solana_client::pool::{AmmConfigEntry, AmmConfigType},
    services::exchange::cache::AMM_CONFIG_CACHE_METRICS,
};

/// Global thread-safe instance of the `AmmConfigCache`.
static AMM_CONFIG_CACHE: LazyLock<RwLock<AmmConfigCache>> =
    LazyLock::new(|| RwLock::new(AmmConfigCache::new()));

/// Returns a global static reference to the AmmConfig cache.
#[must_use]
pub fn get_amm_config_cache() -> &'static RwLock<AmmConfigCache> {
    &AMM_CONFIG_CACHE
}

/// A high-performance, thread-safe cache for Automated Market Maker (AMM) configurations.
#[derive(Debug, Default)]
pub struct AmmConfigCache {
    /// Internal storage mapping the configuration address ([`Pubkey`]) to its generic type.
    inner: AHashMap<Pubkey, AmmConfigType>,
}

impl AmmConfigCache {
    /// Creates a new `AmmConfigCache`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: AHashMap::with_capacity(128),
        }
    }

    /// Inserts a protocol-specific configuration into the cache.
    #[inline]
    pub fn insert<T: AmmConfigEntry>(&mut self, key: Pubkey, config: T) {
        let dex = config.dex_name();
        let previous = self.inner.insert(key, config.into());

        if previous.is_none() {
            AMM_CONFIG_CACHE_METRICS.record(dex)
        }
    }

    /// Retrieves a specific configuration type from the cache by its address.
    #[inline]
    #[must_use]
    pub fn get(&self, key: &Pubkey) -> Option<AmmConfigType> {
        self.inner.get(key).copied()
    }

    /// Retrieves a specific configuration type from the cache by its address.
    #[inline]
    #[must_use]
    pub fn get_typed<T: AmmConfigEntry>(&self, key: &Pubkey) -> Option<T> {
        T::extract(self.inner.get(key)?).copied()
    }

    /// Returns the total number of configurations currently stored in the cache.
    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Returns `true` if the cache contains no configurations.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}
