use ahash::AHashMap;
use solana_sdk::pubkey::Pubkey;

use crate::{libs::solana_client::pool::DexPool, services::exchange::cache::POOL_CACHE_METRICS};

/// Represents a normalized pair of two token mints.
/// Normalization ensures that (MintA, MintB) is equivalent to (MintB, MintA).
#[derive(Hash, PartialEq, Eq, Clone, Copy)]
pub struct TokenPair {
    pub mint_a: Pubkey,
    pub mint_b: Pubkey,
}

impl TokenPair {
    /// Creates a new TokenPair, automatically sorting mints to maintain
    /// a consistent internal order regardless of input sequence.
    #[must_use]
    pub fn new(a: Pubkey, b: Pubkey) -> Self {
        if a < b {
            Self {
                mint_a: a,
                mint_b: b,
            }
        } else {
            Self {
                mint_b: b,
                mint_a: a,
            }
        }
    }
}

/// Thread-safe registry for DEX pool logic handlers (calculators).
/// Stores implementations of the DexPool trait, enabling real-time
/// swap simulations for various protocols.
pub struct PoolCache {
    /// Internal map linking pool Pubkeys to their respective logic providers.
    data: AHashMap<Pubkey, Box<dyn DexPool>>,

    /// Index mapping a TokenPair to all available pool addresses for that pair.
    pair_index: AHashMap<TokenPair, Vec<Pubkey>>,
}

impl Default for PoolCache {
    fn default() -> Self {
        Self::new()
    }
}

impl PoolCache {
    #[must_use]
    pub fn new() -> Self {
        Self {
            data: AHashMap::with_capacity(1024),
            pair_index: AHashMap::with_capacity(1024),
        }
    }

    /// Updates or inserts a pool into the cache and refreshes the pair index.
    pub fn update(&mut self, pool_id: Pubkey, pool: Box<dyn DexPool>) {
        let (a, b) = pool.get_mints();
        let pair = TokenPair::new(a, b);

        // Update the index: add pool_id if it's not already tracked for this pair
        let pool_ids = self.pair_index.entry(pair).or_default();
        if !pool_ids.contains(&pool_id) {
            pool_ids.push(pool_id);
        }

        if !self.data.contains_key(&pool_id) {
            POOL_CACHE_METRICS.inc(pool.dex_name());
        }

        self.data.insert(pool_id, pool);
    }

    /// Returns an iterator over all pool logic providers for a given pair of mints.
    /// The order of 'a' and 'b' does not matter due to TokenPair normalization.
    pub fn get_pair_pools(&self, a: Pubkey, b: Pubkey) -> impl Iterator<Item = &dyn DexPool> {
        let pair = TokenPair::new(a, b);

        self.pair_index
            .get(&pair)
            .into_iter()
            .flat_map(|ids| ids.iter())
            .filter_map(|id| self.data.get(id).map(|p| p.as_ref()))
    }

    /// Returns an iterator over all tracked pools in the cache.
    pub fn values(&self) -> impl Iterator<Item = &dyn DexPool> {
        self.data.values().map(|p| p.as_ref())
    }

    #[must_use]
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Returns the total number of pools currently stored in the cache.
    #[must_use]
    #[inline]
    pub fn len(&self) -> usize {
        self.data.len()
    }
}
