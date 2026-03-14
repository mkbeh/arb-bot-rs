use std::sync::OnceLock;

use ahash::AHashSet;
use parking_lot::RwLock;
use solana_sdk::{account::Account, pubkey::Pubkey};
use tracing::warn;

use crate::{
    libs::solana_client::{
        dex::utils::parse_vault_amount,
        metrics::DexMetrics,
        models::{Event, PoolState},
        pool::*,
    },
    services::exchange::cache::{
        LiquidityCache, LiquidityIndex, LiquidityIndexCache, PoolCache, VaultCache,
    },
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
    /// Pool IDs that were updated.
    pub changed_pools: AHashSet<Pubkey>,
    /// Vault pubkeys that need amount refresh via RPC.
    pub vaults: AHashSet<Pubkey>,
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
    pub indices: LiquidityIndexCache,
    pub liquidity: LiquidityCache,
    pub pools: PoolCache,
    pub vaults: VaultCache,
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
        }
    }

    /// Processes a batch of raw on-chain account events and updates the market state.
    pub fn update_states(&mut self, events: Vec<Event>) -> MarketUpdateResult {
        let mut result = MarketUpdateResult::default();

        for event in events {
            let Event::Account(acc) = event else {
                continue;
            };

            let Some(updated) = self.update_state(acc.pubkey, acc.pool_state) else {
                continue;
            };

            if let Some(vaults) = updated.vaults {
                result.vaults.extend(vaults);
            }

            result.changed_pools.insert(updated.pool_id);
        }

        result
    }

    /// Updates the vault token balances in the market state
    pub fn update_vaults(&mut self, vaults: &[Pubkey], accounts: Vec<Option<Account>>) {
        for (pubkey, account) in vaults.iter().zip(accounts.into_iter()) {
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
}
