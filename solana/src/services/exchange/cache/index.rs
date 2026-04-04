use ahash::AHashMap;
use solana_sdk::pubkey::Pubkey;

use crate::{
    libs::solana_client::{
        metrics::{DEX_METEORA_DLMM, DEX_ORCA, DEX_RAYDIUM_CLMM, ProtocolMetrics},
        models::PoolState,
    },
    services::exchange::cache::INDEX_CACHE_METRICS,
};

/// Represents the current price coordinate (index) on the protocol's grid.
/// This index serves as a reference point for sliding window liquidity caching.
#[derive(Debug, Clone, Copy)]
pub enum LiquidityIndex {
    /// Meteora DLMM: uses active_id to identify the current bin.
    MeteoraDlmm {
        /// The ID of the bin where the current price is located.
        active_id: i32,
    },
    /// Orca Whirlpool: uses tick indices and spacing for price positioning.
    Orca {
        /// The index of the tick where the current price is located
        tick_current_index: i32,
        /// Distance between initialized ticks, required for array boundary calculations.
        tick_spacing: u16,
    },
    /// Raydium CLMM: similar to Orca, uses ticks for concentrated liquidity.
    RaydiumClmm {
        /// The index of the tick where the current price is located.
        tick_current: i32,
        /// Distance between initialized ticks.
        tick_spacing: u16,
    },
}

/// A lightweight registry that tracks the current price position (index) of each monitored pool.
/// This storage provides the "center" coordinate needed by LiquidityStorage to manage its sliding
/// window.
pub struct LiquidityIndexCache {
    data: AHashMap<Pubkey, LiquidityIndex>,
}

impl Default for LiquidityIndexCache {
    fn default() -> Self {
        Self::new()
    }
}

impl LiquidityIndexCache {
    #[must_use]
    pub fn new() -> Self {
        Self {
            data: AHashMap::with_capacity(1024),
        }
    }

    /// Updates or inserts the latest liquidity index for a specific pool.
    pub fn update(&mut self, pool_id: Pubkey, index: LiquidityIndex) {
        let prev = self.data.insert(pool_id, index);
        if prev.is_none() {
            INDEX_CACHE_METRICS.record(index.name());
        }
    }

    /// Retrieves the current liquidity index for a given pool ID.
    #[must_use]
    pub fn get(&self, pool_id: &Pubkey) -> Option<&LiquidityIndex> {
        self.data.get(pool_id)
    }
}

impl ProtocolMetrics for LiquidityIndex {
    fn name(&self) -> &'static str {
        match self {
            Self::MeteoraDlmm { .. } => DEX_METEORA_DLMM,
            Self::Orca { .. } => DEX_ORCA,
            Self::RaydiumClmm { .. } => DEX_RAYDIUM_CLMM,
        }
    }
}

/// Conversions for extracting price indices from pool states (by reference)
impl TryFrom<&PoolState> for LiquidityIndex {
    type Error = ();

    fn try_from(state: &PoolState) -> Result<Self, Self::Error> {
        match state {
            PoolState::LbPairMeteoraDlmm(s) => Ok(Self::MeteoraDlmm {
                active_id: s.active_id,
            }),
            PoolState::WhirlpoolOrca(s) => Ok(Self::Orca {
                tick_current_index: s.tick_current_index,
                tick_spacing: s.tick_spacing,
            }),
            PoolState::PoolStateRaydiumClmm(s) => Ok(Self::RaydiumClmm {
                tick_current: s.tick_current,
                tick_spacing: s.tick_spacing,
            }),
            _ => Err(()),
        }
    }
}
