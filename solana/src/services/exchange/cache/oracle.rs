use ahash::AHashMap;
use solana_sdk::pubkey::Pubkey;

use crate::{
    libs::solana_client::dex::orca::Oracle, services::exchange::cache::ORACLE_CACHE_METRICS,
};

/// Cache for Orca Oracle accounts keyed by whirlpool pubkey.
pub struct OracleCache {
    data: AHashMap<Pubkey, Oracle>,
}

impl Default for OracleCache {
    fn default() -> Self {
        Self::new()
    }
}

impl OracleCache {
    #[must_use]
    pub fn new() -> Self {
        Self {
            data: AHashMap::with_capacity(1024),
        }
    }

    pub fn update(&mut self, whirlpool: Pubkey, oracle: Oracle) {
        self.data.insert(whirlpool, oracle);
        ORACLE_CACHE_METRICS.set_cache_size(self.data.len());
    }

    #[inline]
    #[must_use]
    pub fn get(&self, whirlpool: &Pubkey) -> Option<&Oracle> {
        self.data.get(whirlpool)
    }
}
