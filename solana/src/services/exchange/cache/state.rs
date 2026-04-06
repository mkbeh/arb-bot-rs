use std::sync::OnceLock;

use ahash::{AHashMap, AHashSet};
use parking_lot::RwLock;
use solana_sdk::{account::Account, clock::Clock, pubkey::Pubkey};
use tracing::warn;

use crate::{
    libs::solana_client::{
        dex::{orca::Oracle, utils::parse_vault_amount},
        metrics::ProtocolMetrics,
        models::*,
        pool::*,
        protocols::kamino::*,
    },
    services::exchange::cache::*,
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

#[derive(Default)]
pub struct MarketUpdateResult {
    /// Pool IDs mapped to their most recent slot number.
    pub changed_pools: AHashMap<Pubkey, u64>,
    /// Vault pubkeys that need amount refresh via RPC.
    pub vaults: AHashSet<Pubkey>,
    /// Pool IDs that were seen for the first time in this batch.
    /// Used to trigger incremental graph reconstruction in the compute service.
    pub new_pools: AHashSet<Pubkey>,
}

impl MarketUpdateResult {
    /// Records a pool update, keeping only the most recent slot for each pool.
    fn record_pool_update(&mut self, pool_id: Pubkey, slot: u64, is_new: bool) {
        let entry = self.changed_pools.entry(pool_id).or_default();
        *entry = (*entry).max(slot);

        if is_new {
            self.new_pools.insert(pool_id);
        }
    }
}

/// Represents the result of a single pool state update.
pub struct UpdatedPool {
    /// The public key of the pool that was updated.
    pub pool_id: Pubkey,
    /// Vault pubkeys requiring a balance.
    ///
    /// Contains `[token_a_vault, token_b_vault]` for pools that use external
    /// vault accounts (e.g. Raydium CPMM, Raydium AMM).
    /// `None` for pools that don't use vaults (e.g. tick array updates).
    pub vaults: Option<[Pubkey; 2]>,
    /// Indicates whether this pool was inserted for the first time.
    pub is_new: bool,
}

/// Root state container for all DEX-related data in the market.
pub struct MarketState {
    indices: LiquidityIndexCache,
    liquidity: LiquidityCache,
    pools: PoolCache,
    vaults: VaultCache,
    oracles: OracleCache,
    bitmaps: BitmapCache,
    reserves: ReserveCache,
    clock: Option<Clock>,
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
            vaults: VaultCache::default(),
            oracles: OracleCache::new(),
            bitmaps: BitmapCache::new(),
            reserves: ReserveCache::new(),
            clock: None,
        }
    }

    /// Processes a batch of raw on-chain account events and updates the market state.
    pub fn update_events(&mut self, events: Vec<Event>) -> MarketUpdateResult {
        let mut result = MarketUpdateResult::default();

        for event in events {
            match event {
                Event::Clock(clock) => self.update_clock(clock),
                Event::Program(acc) => {
                    let slot = acc.slot;

                    let Some(updated) = self.update_state(acc.pubkey, acc.pool_state) else {
                        continue;
                    };

                    if let Some(vaults) = updated.vaults {
                        result.vaults.extend(vaults);
                    }

                    result.record_pool_update(acc.pubkey, slot, updated.is_new);
                }
                _ => {}
            }
        }

        result
    }

    /// Updates the vault token balances in the market state
    pub fn update_vaults(&mut self, vaults: &[Pubkey], accounts: Vec<Option<Account>>) {
        for (pubkey, account) in vaults.iter().zip(accounts) {
            let Some(acc) = account else {
                continue;
            };

            match parse_vault_amount(&acc.data) {
                Ok(amount) => self.vaults.update(*pubkey, amount),
                Err(e) => {
                    warn!("Failed to parse vault amount for {pubkey}: {e}");
                }
            }
        }
    }

    /// Dispatches an incoming `PoolState` update to the appropriate cache.
    fn update_state(&mut self, pool_id: Pubkey, state: PoolState) -> Option<UpdatedPool> {
        if let Ok(idx) = LiquidityIndex::try_from(&state) {
            self.indices.update(pool_id, idx);
        }

        match state {
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

            PoolState::LbPairMeteoraDlmm(s) => self.update_pool(pool_id, s),
            PoolState::WhirlpoolOrca(s) => self.update_pool(pool_id, s),
            PoolState::PoolStateRaydiumClmm(s) => self.update_pool(pool_id, s),
            PoolState::PoolMeteoraDammV2(s) => self.update_pool(pool_id, s),
            PoolState::PoolStateRaydiumCpmm(s) => self.update_pool(pool_id, s),
            PoolState::AmmInfoRaydiumAmm(s) => self.update_pool(pool_id, s),

            PoolState::BinArrayBitmapExtensionMeteoraDlmm(b) => {
                self.update_bitmap(pool_id, CachedBitmap::MeteoraDlmm(b))
            }
            PoolState::TickArrayBitmapExtensionRadiumClmm(b) => {
                self.update_bitmap(pool_id, CachedBitmap::RaydiumClmm(b))
            }

            PoolState::OracleOrca(s) => self.update_oracle(&s),
            PoolState::ReserveKamino(s) => self.update_reserve(&s),

            PoolState::BondingCurvePumpFun(_) => None,

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
    fn update_liquidity(&mut self, pool_id: Pubkey, array: LiquidityArray) -> Option<UpdatedPool> {
        if let Some(index) = self.indices.get(&pool_id).copied() {
            self.liquidity.update(pool_id, array, &index);
            return Some(UpdatedPool {
                pool_id,
                vaults: None,  // Ticks never trigger vault refresh
                is_new: false, // Liquidity updates never represent new pools
            });
        }
        None
    }

    /// Stores a new pool logic provider (DexPool) into the pool cache.
    fn update_pool<T: DexPool + ProtocolMetrics + 'static>(
        &mut self,
        pool_id: Pubkey,
        pool: Box<T>,
    ) -> Option<UpdatedPool> {
        let vaults = pool.get_vault_pubkeys().map(|(v0, v1)| [v0, v1]);
        let is_new = self.pools.update(pool_id, pool);
        Some(UpdatedPool {
            pool_id,
            vaults,
            is_new,
        })
    }

    fn update_oracle(&mut self, oracle: &Oracle) -> Option<UpdatedPool> {
        self.oracles.update(oracle.pubkey(), *oracle);
        None
    }

    fn update_bitmap(&mut self, pool_id: Pubkey, bitmap: CachedBitmap) -> Option<UpdatedPool> {
        self.bitmaps.update(pool_id, bitmap);
        None
    }

    fn update_reserve(&mut self, reserve: &Reserve) -> Option<UpdatedPool> {
        self.reserves.update(reserve);
        None
    }

    fn update_clock(&mut self, clock: Clock) {
        SYSTEM_CACHE_METRICS.record_clock(clock.slot, clock.unix_timestamp);
        self.clock = Some(clock);
    }

    #[inline]
    #[must_use]
    pub fn pools(&self) -> &PoolCache {
        &self.pools
    }

    #[inline]
    #[must_use]
    pub fn liquidity(&self) -> &LiquidityCache {
        &self.liquidity
    }

    #[inline]
    #[must_use]
    pub fn reserves(&self) -> &ReserveCache {
        &self.reserves
    }

    #[inline]
    #[must_use]
    pub fn vaults(&self) -> &VaultCache {
        &self.vaults
    }

    #[inline]
    #[must_use]
    pub fn oracles(&self) -> &OracleCache {
        &self.oracles
    }

    #[inline]
    #[must_use]
    pub fn bitmaps(&self) -> &BitmapCache {
        &self.bitmaps
    }

    #[must_use]
    pub fn clock(&self) -> Option<&Clock> {
        self.clock.as_ref()
    }
}
