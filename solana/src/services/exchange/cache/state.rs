use std::sync::OnceLock;

use parking_lot::RwLock;
use solana_sdk::pubkey::Pubkey;
use tracing::warn;

use crate::{
    libs::solana_client::{
        metrics::DexMetrics,
        models::PoolState,
        pool::{DexPool, traits::LiquidityArray},
    },
    services::exchange::cache::{LiquidityCache, LiquidityIndex, LiquidityIndexCache, PoolCache},
};

/// Global root state container.
static MARKET_STATE: OnceLock<RwLock<MarketState>> = OnceLock::new();

pub fn init_market_state(depth: i64) -> anyhow::Result<()> {
    let state = RwLock::new(MarketState::new(depth));
    MARKET_STATE
        .set(state)
        .map_err(|_| anyhow::anyhow!("MarketState already initialized"))
}

pub fn get_market_state() -> &'static RwLock<MarketState> {
    MARKET_STATE.get().expect("MarketState not initialized")
}

/// Root state container for all DEX-related data in the market.
pub struct MarketState {
    pub indices: LiquidityIndexCache,
    pub liquidity: LiquidityCache,
    pub pools: PoolCache,
}

impl MarketState {
    /// Initializes a new MarketState with a specific depth for liquidity caching.
    ///
    /// # Arguments
    /// * `depth` - The number of "pages" (arrays) to keep in memory on either side of the current
    ///   price.
    #[must_use]
    pub fn new(depth: i64) -> Self {
        Self {
            indices: LiquidityIndexCache::default(),
            liquidity: LiquidityCache::new(depth),
            pools: PoolCache::default(),
        }
    }

    /// Dispatches an incoming `PoolState` update to the appropriate cache.
    pub fn update_state(&mut self, pool_id: Pubkey, state: PoolState) -> Option<Pubkey> {
        if let Ok(idx) = LiquidityIndex::try_from(&state) {
            self.indices.update(pool_id, idx);
        }

        match state {
            // --- LIQUIDITY ARRAYS (Bins/Ticks) ---
            PoolState::BinArrayMeteoraDlmm(s) => {
                self.update_liquidity(s.pubkey(), LiquidityArray::MeteoraDlmm(*s))
            }
            PoolState::FixedTickArrayOrca(s) => {
                self.update_liquidity(s.pubkey(), LiquidityArray::OrcaFixed(*s))
            }
            PoolState::DynamicTickArrayOrca(s) => {
                self.update_liquidity(s.pubkey(), LiquidityArray::OrcaDynamic(*s))
            }
            PoolState::TickArrayStateRaydiumClmm(s) => {
                self.update_liquidity(s.pubkey(), LiquidityArray::RaydiumClmm(*s))
            }

            // --- POOL CALCULATORS (Logic & Reserves) ---
            PoolState::LbPairMeteoraDlmm(s) => self.update_pool(pool_id, s),
            PoolState::WhirlpoolOrca(s) => self.update_pool(pool_id, s),
            PoolState::PoolStateRaydiumClmm(s) => self.update_pool(pool_id, s),
            PoolState::PoolMeteoraDammV2(s) => self.update_pool(pool_id, s),
            PoolState::PoolStateRaydiumCpmm(s) => self.update_pool(pool_id, s),
            PoolState::AmmInfoRaydiumAmm(s) => self.update_pool(pool_id, s),

            // --- IGNORED STATES ---
            PoolState::BondingCurvePumpFun(_)
            | PoolState::BinArrayBitmapExtensionMeteoraDlmm(_)
            | PoolState::TickArrayBitmapExtensionRadiumClmm(_) => None,

            PoolState::Unknown(_) => {
                warn!("Unknown PoolState for pool: {}", pool_id);
                None
            }
        }
    }

    /// Processes a new liquidity array update.
    ///
    /// This method acts as a coordinator: it first retrieves the current price index
    /// for the given pool to determine if the new liquidity data is within the
    /// required depth before storing it in the liquidity cache.
    fn update_liquidity(&mut self, pool_id: Pubkey, array: LiquidityArray) -> Option<Pubkey> {
        // Only update liquidity if we have a known price index for this pool.
        // This prevents caching irrelevant data for pools we aren't tracking indices for.
        if let Some(index) = self.indices.get(&pool_id).copied() {
            self.liquidity.update(pool_id, array, &index);
            return Some(pool_id);
        }
        None
    }

    /// Stores a new pool logic provider (DexPool) into the pool cache.
    fn update_pool<T: DexPool + DexMetrics + 'static>(
        &mut self,
        pool_id: Pubkey,
        pool: Box<T>,
    ) -> Option<Pubkey> {
        self.pools.update(pool_id, pool);
        Some(pool_id)
    }
}
