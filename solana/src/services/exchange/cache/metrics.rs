use std::sync::LazyLock;

use metrics::{Unit, counter, describe_counter, describe_gauge, gauge};
use solana_sdk::pubkey::Pubkey;

use crate::libs::solana_client::{metrics::LBL_DEX, utils};

/// Global metrics provider for the Pool Logic Cache.
pub static POOL_CACHE_METRICS: LazyLock<PoolCacheMetrics> = LazyLock::new(PoolCacheMetrics::new);

/// Global metrics provider for the Liquidity Depth Cache.
pub static LIQUIDITY_CACHE_METRICS: LazyLock<LiquidityCacheMetrics> =
    LazyLock::new(LiquidityCacheMetrics::new);

/// Global metrics provider for the Mint Cache.
pub static MINT_CACHE_METRICS: LazyLock<MintCacheMetrics> = LazyLock::new(MintCacheMetrics::new);

/// Global metrics provider for the Amm Config Cache.
pub static AMM_CONFIG_CACHE_METRICS: LazyLock<AmmConfigCacheMetrics> =
    LazyLock::new(AmmConfigCacheMetrics::new);

/// Global metrics provider for the Vault Cache.
pub static VAULT_CACHE_METRICS: LazyLock<VaultCacheMetrics> = LazyLock::new(VaultCacheMetrics::new);

/// Global metrics provider for the Oracle Cache.
pub static ORACLE_CACHE_METRICS: LazyLock<OracleCacheMetrics> =
    LazyLock::new(OracleCacheMetrics::new);

/// Global metrics provider for the Bitmap Cache.
pub static BITMAP_CACHE_METRICS: LazyLock<BitmapCacheMetrics> =
    LazyLock::new(BitmapCacheMetrics::new);

/// Global metrics provider for the Reserve Cache.
pub static RESERVE_CACHE_METRICS: LazyLock<ReserveCacheMetrics> =
    LazyLock::new(ReserveCacheMetrics::new);

/// Global metrics provider for the System Cache.
pub static SYSTEM_CACHE_METRICS: LazyLock<SystemCacheMetrics> =
    LazyLock::new(SystemCacheMetrics::new);

/// Global metrics provider for the Pool Sync Cache.
pub static POOL_SYNC_CACHE_METRICS: LazyLock<PoolSyncCacheMetrics> =
    LazyLock::new(PoolSyncCacheMetrics::new);

pub fn init_cache_metrics() {
    let _ = &*POOL_CACHE_METRICS;
    let _ = &*LIQUIDITY_CACHE_METRICS;
    let _ = &*MINT_CACHE_METRICS;
    let _ = &*AMM_CONFIG_CACHE_METRICS;
    let _ = &*VAULT_CACHE_METRICS;
    let _ = &*ORACLE_CACHE_METRICS;
    let _ = &*BITMAP_CACHE_METRICS;
    let _ = &*RESERVE_CACHE_METRICS;
    let _ = &*SYSTEM_CACHE_METRICS;
    let _ = &*POOL_SYNC_CACHE_METRICS;
}

/// Metrics manager for tracking DEX pool implementations.
pub struct PoolCacheMetrics;

impl PoolCacheMetrics {
    const METRIC_POOL_CACHE_SIZE: &str = "cache_size_pool_total";

    /// Initializes and registers descriptions for pool-related metrics.
    fn new() -> Self {
        describe_counter!(
            Self::METRIC_POOL_CACHE_SIZE,
            Unit::Count,
            "Total pools tracked in cache"
        );

        Self
    }

    /// Increments the total count of registered pools for a specific DEX.
    #[inline]
    pub fn record(&self, dex: &'static str) {
        let labels = &[(LBL_DEX, dex)];
        counter!(Self::METRIC_POOL_CACHE_SIZE, labels).increment(1);
    }
}

/// Metrics manager for tracking liquidity depth (tick/bin arrays).
pub struct LiquidityCacheMetrics;

impl LiquidityCacheMetrics {
    const METRIC_LIQUIDITY_CACHE_SIZE: &str = "cache_size_liquidity_total";

    /// Initializes and registers descriptions for liquidity-related metrics.
    fn new() -> Self {
        describe_gauge!(
            Self::METRIC_LIQUIDITY_CACHE_SIZE,
            Unit::Count,
            "Current liquidity arrays in memory"
        );
        Self
    }

    /// Sets the absolute number of liquidity arrays currently stored for a specific DEX.
    #[inline]
    pub fn set_liquidity(&self, dex: &'static str, value: usize) {
        let labels = &[(LBL_DEX, dex)];
        gauge!(Self::METRIC_LIQUIDITY_CACHE_SIZE, labels).set(value as f64);
    }
}

/// Metrics manager for tracking cached mint accounts.
pub struct MintCacheMetrics;

impl Default for MintCacheMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl MintCacheMetrics {
    const METRIC_MINT_CACHE_SIZE: &str = "cache_size_mint_total";

    /// Initializes and registers descriptions for mint cache metrics.
    #[must_use]
    pub fn new() -> Self {
        describe_gauge!(
            Self::METRIC_MINT_CACHE_SIZE,
            Unit::Count,
            "The current total number of mint accounts tracked in the cache"
        );

        Self
    }

    /// Sets the current total count of elements in the cache.
    ///
    /// This should be called after batch updates to reflect the current state.
    #[inline]
    pub fn set_cache_size(&self, size: usize) {
        gauge!(Self::METRIC_MINT_CACHE_SIZE).set(size as f64);
    }
}

/// Metrics for tracking the state and activity of the AMM configuration cache.
pub struct AmmConfigCacheMetrics;

impl Default for AmmConfigCacheMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl AmmConfigCacheMetrics {
    const METRIC_AMM_CONFIG_CACHE_SIZE: &str = "cache_size_amm_config_total";

    /// Initializes and registers descriptions for cache metrics.
    #[must_use]
    pub fn new() -> Self {
        describe_counter!(
            Self::METRIC_AMM_CONFIG_CACHE_SIZE,
            Unit::Count,
            "The total number of AMM configs inserted into the cache, by protocol"
        );

        Self
    }

    /// Increments the AMM configuration cache counter for a specific DEX.
    #[inline]
    pub fn record(&self, dex: &'static str) {
        let labels = &[(LBL_DEX, dex)];
        counter!(Self::METRIC_AMM_CONFIG_CACHE_SIZE, labels).increment(1);
    }
}

/// Metrics manager for tracking vault amounts.
pub struct VaultCacheMetrics;

impl Default for VaultCacheMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl VaultCacheMetrics {
    const METRIC_VAULT_CACHE_SIZE: &str = "cache_size_vault_total";

    /// Initializes and registers descriptions for cache metrics.
    #[must_use]
    pub fn new() -> Self {
        describe_gauge!(
            Self::METRIC_VAULT_CACHE_SIZE,
            Unit::Count,
            "The current total number of vault amounts tracked in the cache"
        );

        Self
    }

    /// Sets the current total count of elements in the cache.
    ///
    /// This should be called after batch updates to reflect the current state.
    #[inline]
    pub fn set_cache_size(&self, size: usize) {
        gauge!(Self::METRIC_VAULT_CACHE_SIZE).set(size as f64);
    }
}

/// Metrics manager for tracking oracles.
pub struct OracleCacheMetrics;

impl Default for OracleCacheMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl OracleCacheMetrics {
    const METRIC_ORACLE_CACHE_SIZE: &str = "cache_size_oracle_total";

    /// Initializes and registers descriptions for cache metrics.
    #[must_use]
    pub fn new() -> Self {
        describe_gauge!(
            Self::METRIC_ORACLE_CACHE_SIZE,
            Unit::Count,
            "The current total number of oracles tracked in the cache"
        );

        Self
    }

    /// Sets the current total count of elements in the cache.
    ///
    /// This should be called after batch updates to reflect the current state.
    #[inline]
    pub fn set_cache_size(&self, size: usize) {
        gauge!(Self::METRIC_ORACLE_CACHE_SIZE).set(size as f64);
    }
}

pub struct BitmapCacheMetrics;

impl Default for BitmapCacheMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl BitmapCacheMetrics {
    const METRIC_BITMAP_CACHE_SIZE: &str = "cache_size_bitmap_total";
    const LBL_POOL: &str = "pool";
    const LBL_PROTOCOL: &str = "protocol";

    #[must_use]
    pub fn new() -> Self {
        describe_gauge!(
            Self::METRIC_BITMAP_CACHE_SIZE,
            Unit::Count,
            "Indicates whether a bitmap extension for the given pool is present in the cache (1 = present)"
        );
        Self
    }

    /// Records that a bitmap for the given pool is present in the cache.
    pub fn record(&self, pool: &Pubkey, protocol: &'static str) {
        gauge!(
            Self::METRIC_BITMAP_CACHE_SIZE,
            Self::LBL_POOL => pool.to_string(),
            Self::LBL_PROTOCOL => protocol
        )
        .set(1.0);
    }
}

pub struct ReserveCacheMetrics;

impl Default for ReserveCacheMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl ReserveCacheMetrics {
    const METRIC_RESERVE_CACHE_SIZE: &str = "cache_size_reserve_total";
    const METRIC_RESERVE_AVAILABLE_AMOUNT: &str = "cache_reserve_available_amount";
    const METRIC_RESERVE_UPDATED_AT: &str = "cache_reserve_updated_at";
    const LBL_MINT: &str = "mint";

    #[must_use]
    pub fn new() -> Self {
        describe_gauge!(
            Self::METRIC_RESERVE_CACHE_SIZE,
            Unit::Count,
            "Indicates whether a reserve for the given mint is present in the cache (1 = present)"
        );
        describe_gauge!(
            Self::METRIC_RESERVE_AVAILABLE_AMOUNT,
            Unit::Count,
            "Total available liquidity amount for the given reserve"
        );
        describe_gauge!(
            Self::METRIC_RESERVE_UPDATED_AT,
            Unit::Seconds,
            "Timestamp of the last reserve cache update in seconds"
        );
        Self
    }

    /// Records that a reserve for the given mint is present in the cache
    /// and updates its available liquidity amount.
    pub fn record(&self, mint: &Pubkey, amount: f64, updated_at: u64) {
        gauge!(Self::METRIC_RESERVE_CACHE_SIZE, Self::LBL_MINT => mint.to_string()).set(1.0);
        gauge!(Self::METRIC_RESERVE_AVAILABLE_AMOUNT, Self::LBL_MINT => mint.to_string())
            .set(amount);
        gauge!(Self::METRIC_RESERVE_UPDATED_AT, Self::LBL_MINT => mint.to_string())
            .set(updated_at as f64);
    }
}

pub struct SystemCacheMetrics;

impl Default for SystemCacheMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl SystemCacheMetrics {
    const METRIC_CLOCK_SLOT: &str = "cache_system_clock_slot";
    const METRIC_CLOCK_TIMESTAMP: &str = "cache_system_clock_unix_timestamp";
    const METRIC_NETWORK_LAG: &str = "cache_system_network_lag";

    #[must_use]
    pub fn new() -> Self {
        describe_gauge!(
            Self::METRIC_CLOCK_SLOT,
            Unit::Count,
            "Current Solana clock slot"
        );
        describe_gauge!(
            Self::METRIC_CLOCK_TIMESTAMP,
            Unit::Seconds,
            "Current Solana clock unix timestamp"
        );
        describe_gauge!(
            Self::METRIC_NETWORK_LAG,
            Unit::Milliseconds,
            "Observed network lag in milliseconds: difference between local time and blockchain time"
        );

        Self
    }

    pub fn record_clock(&self, slot: u64, timestamp: i64) {
        let now_ms = utils::get_timestamp_ms();
        let clock_ms = (timestamp as u64).saturating_mul(1000);
        let lag_ms = now_ms.saturating_sub(clock_ms);

        gauge!(Self::METRIC_CLOCK_SLOT).set(slot as f64);
        gauge!(Self::METRIC_CLOCK_TIMESTAMP).set(timestamp as f64);
        gauge!(Self::METRIC_NETWORK_LAG).set(lag_ms as f64);
    }
}

pub struct PoolSyncCacheMetrics;

impl Default for PoolSyncCacheMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl PoolSyncCacheMetrics {
    const METRIC_POOL_SYNC_COUNT: &str = "cache_pool_sync_count";
    const LBL_STATUS: &str = "status";
    const STATUS_PENDING: &str = "pending";
    const STATUS_READY: &str = "ready";

    #[must_use]
    pub fn new() -> Self {
        describe_gauge!(
            Self::METRIC_POOL_SYNC_COUNT,
            Unit::Count,
            "Number of pools in each sync status"
        );
        Self
    }

    pub fn set_pending(&self) {
        gauge!(Self::METRIC_POOL_SYNC_COUNT, Self::LBL_STATUS => Self::STATUS_PENDING)
            .increment(1.0);
    }

    pub fn set_ready(&self) {
        gauge!(Self::METRIC_POOL_SYNC_COUNT, Self::LBL_STATUS => Self::STATUS_PENDING)
            .decrement(1.0);
        gauge!(Self::METRIC_POOL_SYNC_COUNT, Self::LBL_STATUS => Self::STATUS_READY).increment(1.0);
    }
}
