use std::sync::OnceLock;

use ahash::{AHashMap, AHashSet};
use parking_lot::RwLock;
use solana_sdk::{account::Account, pubkey::Pubkey};
use tracing::warn;

use crate::{
    libs::solana_client::{
        dex::{orca::Oracle, utils::parse_vault_amount},
        metrics::DexMetrics,
        models::{Event, PoolState},
        pool::*,
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
}

impl MarketUpdateResult {
    /// Records a pool update, keeping only the most recent slot for each pool.
    fn record_pool_update(&mut self, pool_id: Pubkey, slot: u64) {
        let entry = self.changed_pools.entry(pool_id).or_default();
        *entry = (*entry).max(slot);
    }
}

/// Represents the result of a single pool state update.
pub struct UpdatedPool {
    /// The public key of the pool that was updated.
    pub pool_id: Pubkey,
    /// Vault pubkeys requiring a balance.
    ///
    /// Contains `[token_a_vault, token_b_vault]` for pools that use external
    /// vault accounts (e.g. Raydium CPMM, Raydium AMM, Orca).
    /// `None` for pools that don't use vaults (e.g. tick array updates).
    pub vaults: Option<[Pubkey; 2]>,
}

/// Root state container for all DEX-related data in the market.
pub struct MarketState {
    indices: LiquidityIndexCache,
    liquidity: LiquidityCache,
    pools: PoolCache,
    vaults: VaultCache,
    oracles: OracleCache,
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
        }
    }

    /// Processes a batch of raw on-chain account events and updates the market state.
    pub fn update_states(&mut self, events: Vec<Event>) -> MarketUpdateResult {
        let mut result = MarketUpdateResult::default();

        for event in events {
            let Event::Account(acc) = event else {
                continue;
            };

            let slot = acc.slot;

            let Some(updated) = self.update_state(acc.pubkey, acc.pool_state) else {
                continue;
            };

            if let Some(vaults) = updated.vaults {
                result.vaults.extend(vaults);
            }

            result.record_pool_update(acc.pubkey, slot);
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

    /// Returns a unique list of all token mints (assets) available across all cached pools.
    #[must_use]
    pub fn get_pool_mints(&self) -> Vec<Pubkey> {
        let mut mints = AHashSet::with_capacity(self.pools.len() * 2);

        for pool in self.pools.values() {
            let (mint_a, mint_b) = pool.get_mints();
            mints.insert(mint_a);
            mints.insert(mint_b);
        }

        mints.into_iter().collect()
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

            PoolState::OracleOrca(s) => self.update_oracle(&s),

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
    fn update_liquidity(&mut self, pool_id: Pubkey, array: LiquidityArray) -> Option<UpdatedPool> {
        if let Some(index) = self.indices.get(&pool_id).copied() {
            self.liquidity.update(pool_id, array, &index);
            return Some(UpdatedPool {
                pool_id,
                vaults: None, // Ticks never trigger vault refresh
            });
        }
        None
    }

    /// Stores a new pool logic provider (DexPool) into the pool cache.
    fn update_pool<T: DexPool + DexMetrics + 'static>(
        &mut self,
        pool_id: Pubkey,
        pool: Box<T>,
    ) -> Option<UpdatedPool> {
        let vaults = pool.get_vault_pubkeys().map(|(v0, v1)| [v0, v1]);
        self.pools.update(pool_id, pool);
        Some(UpdatedPool { pool_id, vaults })
    }

    /// Stores a new Orca Oracle account into the oracle cache.
    fn update_oracle(&mut self, oracle: &Oracle) -> Option<UpdatedPool> {
        self.oracles.update(oracle.pubkey(), *oracle);
        None
    }
}
