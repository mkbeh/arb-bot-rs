use std::collections::BTreeMap;

use ahash::AHashMap;
use solana_sdk::pubkey::Pubkey;

use crate::{
    libs::solana_client::{dex::*, metrics::*, pool::*},
    services::exchange::cache::*,
};

/// Trait for types that represent a chunk of liquidity (e.g., TickArray or BinArray).
/// Defines how to extract a unique key used to identify and sort the array
/// within a pool's liquidity map (e.g., start tick index or bin index).
pub trait LiquidityUpdate: Sized {
    /// Unique identifier for the array (usually the start tick or bin index).
    type Key: Ord + Copy;

    /// Returns the unique key of the current array.
    fn get_key(&self) -> Self::Key;
}

/// Global cache holding liquidity arrays for different DEX protocols.
pub struct LiquidityCache {
    meteora: SubCache<i64, meteora_dlmm::BinArray>,
    raydium: SubCache<i32, raydium_clmm::TickArrayState>,
    orca: SubCache<i32, orca::OrcaTickArray>,
}

impl Default for LiquidityCache {
    fn default() -> Self {
        Self::new()
    }
}

impl LiquidityCache {
    #[must_use]
    pub fn new() -> Self {
        Self {
            meteora: SubCache::new(),
            raydium: SubCache::new(),
            orca: SubCache::new(),
        }
    }

    pub fn update(&mut self, pool_id: Pubkey, slot: u64, array: LiquidityArray) {
        match array {
            LiquidityArray::MeteoraDlmm(item) => self.meteora.update(pool_id, slot, item),
            LiquidityArray::RaydiumClmm(item) => self.raydium.update(pool_id, slot, item),
            LiquidityArray::Orca(item) => self.orca.update(pool_id, slot, item),
        }
    }

    #[must_use]
    pub fn get_map(&self, pool_id: &Pubkey, protocol: ProtocolKind) -> Option<LiquidityMap<'_>> {
        match protocol {
            ProtocolKind::MeteoraDlmm => self.meteora.get_liquidity(pool_id),
            ProtocolKind::RaydiumClmm => self.raydium.get_liquidity(pool_id),
            ProtocolKind::Orca => self.orca.get_liquidity(pool_id),
            _ => None,
        }
    }
}

/// Specialized cache for a specific protocol's liquidity arrays.
pub struct SubCache<K, T> {
    /// Maps Pool address to a sorted map of its liquidity arrays.
    data: AHashMap<Pubkey, BTreeMap<K, T>>,
    /// Tracks the last seen slot for each liquidity array per pool.
    slots: AHashMap<Pubkey, AHashMap<K, u64>>,
}

impl<K, T> Default for SubCache<K, T>
where
    K: Ord + Copy + std::hash::Hash,
    T: LiquidityUpdate<Key = K> + ProtocolMetrics,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<K, T> SubCache<K, T>
where
    K: Ord + Copy + std::hash::Hash,
    T: LiquidityUpdate<Key = K> + ProtocolMetrics,
{
    #[must_use]
    pub fn new() -> Self {
        Self {
            data: AHashMap::with_capacity(1024),
            slots: AHashMap::with_capacity(1024),
        }
    }

    pub fn update(&mut self, pool_id: Pubkey, slot: u64, array: T) {
        let dex = array.protocol_name();
        let key = array.get_key();

        let pool_slots = self.slots.entry(pool_id).or_default();
        if let Some(existing_slot) = pool_slots.get(&key)
            && slot <= *existing_slot
        {
            return;
        }

        pool_slots.insert(key, slot);
        self.data.entry(pool_id).or_default().insert(key, array);

        LIQUIDITY_CACHE_METRICS.set_liquidity(dex, self.data.len());
    }

    #[must_use]
    pub fn get_liquidity<'a>(&'a self, pool_id: &Pubkey) -> Option<LiquidityMap<'a>>
    where
        T: IntoLiquidityMap<'a, Key = K>,
    {
        self.data.get(pool_id).map(|m| T::wrap_to_map(m))
    }
}

// --- Protocol Implementations ---

impl LiquidityUpdate for meteora_dlmm::BinArray {
    type Key = i64;

    fn get_key(&self) -> Self::Key {
        self.index
    }
}

impl LiquidityUpdate for raydium_clmm::TickArrayState {
    type Key = i32;

    fn get_key(&self) -> Self::Key {
        self.start_tick_index
    }
}

impl LiquidityUpdate for orca::OrcaTickArray {
    type Key = i32;

    fn get_key(&self) -> Self::Key {
        match self {
            Self::Fixed(a) => a.start_tick_index,
            Self::Dynamic(a) => a.start_tick_index,
        }
    }
}
