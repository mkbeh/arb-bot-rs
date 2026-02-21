use std::collections::BTreeMap;

use ahash::AHashMap;
use solana_sdk::pubkey::Pubkey;

use crate::{
    libs::solana_client::{
        dex::{meteora_dlmm, orca, raydium_clmm},
        metrics::DexMetrics,
        pool::traits::{IntoLiquidityMap, LiquidityArray, LiquidityMap},
    },
    services::exchange::cache::{LIQUIDITY_CACHE_METRICS, LiquidityIndex},
};

/// Trait for types that represent a chunk of liquidity (e.g., TickArray or BinArray).
/// Defines how to identify and position the array relative to the current price.
pub trait LiquidityUpdate: Sized {
    /// Unique identifier for the array (usually the start tick or bin index).
    type Key: Ord + Copy;

    /// Returns the unique key of the current array.
    fn get_key(&self) -> Self::Key;

    /// Calculates pagination information for the array based on the current pool state.
    ///
    /// Returns: `Option<(array_page, current_price_page, scale)>`
    /// - `array_page`: The index of this array in "pages" units.
    /// - `current_price_page`: The index of the page where the current price is located.
    /// - `scale`: Number of units (ticks/bins) per page.
    fn calculate_paging(&self, index: &LiquidityIndex) -> Option<(i64, i64, i64)>;
}

/// Global cache holding liquidity arrays for different DEX protocols.
pub struct LiquidityCache {
    pub meteora: SubCache<i64, meteora_dlmm::BinArray>,
    pub raydium: SubCache<i32, raydium_clmm::TickArrayState>,
    pub orca_fixed: SubCache<i32, orca::FixedTickArray>,
    pub orca_dynamic: SubCache<i32, orca::DynamicTickArray>,
}

impl LiquidityCache {
    /// Creates a new cache instance with a specified depth (radius of pages to keep).
    #[must_use]
    pub fn new(depth: i64) -> Self {
        Self {
            meteora: SubCache::new(depth),
            raydium: SubCache::new(depth),
            orca_fixed: SubCache::new(depth),
            orca_dynamic: SubCache::new(depth),
        }
    }

    pub fn update(&mut self, pool_id: Pubkey, array: LiquidityArray, index: &LiquidityIndex) {
        match array {
            LiquidityArray::MeteoraDlmm(item) => self.meteora.update(pool_id, item, index),
            LiquidityArray::RaydiumClmm(item) => self.raydium.update(pool_id, item, index),
            LiquidityArray::OrcaFixed(item) => self.orca_fixed.update(pool_id, item, index),
            LiquidityArray::OrcaDynamic(item) => self.orca_dynamic.update(pool_id, item, index),
        }
    }
}

/// Specialized cache for a specific protocol's liquidity arrays.
pub struct SubCache<K, T> {
    /// Maps Pool address to a sorted map of its liquidity arrays.
    pub data: AHashMap<Pubkey, BTreeMap<K, T>>,
    /// Max distance (in pages) from the current price page to keep in memory.
    pub depth: i64,
}

impl<K, T> SubCache<K, T>
where
    K: Ord + Copy + Into<i64>,
    T: LiquidityUpdate<Key = K> + Clone,
{
    #[must_use]
    pub fn new(depth: i64) -> Self {
        Self {
            data: AHashMap::with_capacity(1024),
            depth,
        }
    }

    /// Processes a new array update: validates its distance from price and caches it.
    pub fn update(&mut self, pool_id: Pubkey, array: T, index: &LiquidityIndex) {
        // Calculate paging metrics using protocol-specific logic.
        let Some((array_page, current_page, scale)) = array.calculate_paging(index) else {
            return;
        };

        // Cache only if the array is within the allowed depth (radius).
        if (array_page - current_page).abs() <= self.depth {
            let cache_size = {
                let cache = self.data.entry(pool_id).or_default();
                cache.insert(array.get_key(), array);

                // Cleanup: remove arrays that are now out of range due to price movement.
                Self::prune_cache(cache, current_page, self.depth, scale);

                cache.len()
            };

            LIQUIDITY_CACHE_METRICS.set_liquidity(index.dex_name(), self.data.len());
            LIQUIDITY_CACHE_METRICS.record_liquidity_density(index.dex_name(), cache_size);
        }
    }

    /// Retrieves all cached liquidity for a specific pool.
    #[must_use]
    pub fn get_liquidity<'a>(&'a self, pool_id: &Pubkey) -> LiquidityMap<'a>
    where
        T: IntoLiquidityMap<'a, Key = K>,
    {
        self.data
            .get(pool_id)
            .map(|m| T::wrap_to_map(m))
            .unwrap_or(LiquidityMap::None)
    }

    /// Removes BTreeMap entries that fall outside the [current_page - depth, current_page + depth]
    /// range. Comparisons are done in raw index units using the scale factor.
    fn prune_cache(cache: &mut BTreeMap<K, T>, current_page: i64, depth: i64, scale: i64) {
        // Calculate absolute index boundaries.
        // Scale accounts for the number of bins/ticks per array (e.g., 70 for Meteora, 88 *
        // spacing for Orca).
        let min_allowed = (current_page - depth) * scale;
        let max_allowed = (current_page + depth) * scale;

        // Retain only entries within the [min_allowed, max_allowed] range.
        cache.retain(|&idx, _| {
            let i: i64 = idx.into();
            i >= min_allowed && i <= max_allowed
        });
    }
}

// --- Protocol Implementations ---

impl LiquidityUpdate for meteora_dlmm::BinArray {
    type Key = i64;

    fn get_key(&self) -> Self::Key {
        self.index
    }

    fn calculate_paging(&self, index: &LiquidityIndex) -> Option<(i64, i64, i64)> {
        if let LiquidityIndex::MeteoraDlmm { active_id } = index {
            // Calculate the current price page.
            let current_page =
                (*active_id as i64).div_euclid(meteora_dlmm::MAX_BINS_PER_ARRAY as i64);
            // Meteora arrays use sequential indexing, so scale is 1.
            Some((self.index, current_page, 1))
        } else {
            None
        }
    }
}

impl LiquidityUpdate for raydium_clmm::TickArrayState {
    type Key = i32;

    fn get_key(&self) -> Self::Key {
        self.start_tick_index
    }

    fn calculate_paging(&self, index: &LiquidityIndex) -> Option<(i64, i64, i64)> {
        if let LiquidityIndex::RaydiumClmm {
            tick_current,
            tick_spacing,
        } = index
        {
            // Calculate total ticks covered by a single array.
            let ticks_per_array = raydium_clmm::TICK_ARRAY_SIZE_USIZE as i64 * *tick_spacing as i64;

            // Map absolute tick indices to "page" numbers.
            let current_page = (*tick_current as i64).div_euclid(ticks_per_array);
            let array_page = (self.start_tick_index as i64).div_euclid(ticks_per_array);

            Some((array_page, current_page, ticks_per_array))
        } else {
            None
        }
    }
}

impl LiquidityUpdate for orca::FixedTickArray {
    type Key = i32;

    fn get_key(&self) -> Self::Key {
        self.start_tick_index
    }

    fn calculate_paging(&self, index: &LiquidityIndex) -> Option<(i64, i64, i64)> {
        if let LiquidityIndex::Orca {
            tick_current_index,
            tick_spacing,
        } = index
        {
            // Calculate total ticks covered by a single array.
            let ticks_per_array = orca::TICK_ARRAY_SIZE as i64 * *tick_spacing as i64;

            // Map absolute tick indices to "page" numbers.
            let current_page = (*tick_current_index as i64).div_euclid(ticks_per_array);
            let array_page = (self.start_tick_index as i64).div_euclid(ticks_per_array);

            Some((array_page, current_page, ticks_per_array))
        } else {
            None
        }
    }
}

impl LiquidityUpdate for orca::DynamicTickArray {
    type Key = i32;

    fn get_key(&self) -> Self::Key {
        self.start_tick_index
    }

    fn calculate_paging(&self, index: &LiquidityIndex) -> Option<(i64, i64, i64)> {
        if let LiquidityIndex::Orca {
            tick_current_index,
            tick_spacing,
        } = index
        {
            // Calculate total ticks covered by a single array.
            let ticks_per_array = orca::TICK_ARRAY_SIZE as i64 * *tick_spacing as i64;

            // Map absolute tick indices to "page" numbers.
            let current_page = (*tick_current_index as i64).div_euclid(ticks_per_array);
            let array_page = (self.start_tick_index as i64).div_euclid(ticks_per_array);

            Some((array_page, current_page, ticks_per_array))
        } else {
            None
        }
    }
}
