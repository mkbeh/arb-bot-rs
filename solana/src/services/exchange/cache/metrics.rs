use std::sync::LazyLock;

use metrics::{
    Unit, counter, describe_counter, describe_gauge, describe_histogram, gauge, histogram,
};
use tools::http::metrics::HttpMetrics;

use crate::libs::solana_client::metrics::LBL_DEX;

/// Global metrics provider for the Liquidity Index Cache.
pub static INDEX_CACHE_METRICS: LazyLock<IndexCacheMetrics> = LazyLock::new(IndexCacheMetrics::new);

/// Global metrics provider for the Pool Logic Cache.
pub static POOL_CACHE_METRICS: LazyLock<PoolCacheMetrics> = LazyLock::new(PoolCacheMetrics::new);

/// Global metrics provider for the Liquidity Depth Cache.
pub static LIQUIDITY_CACHE_METRICS: LazyLock<LiquidityCacheMetrics> =
    LazyLock::new(LiquidityCacheMetrics::new);

pub fn init_metrics() {
    let _ = &*INDEX_CACHE_METRICS;
    let _ = &*POOL_CACHE_METRICS;
    let _ = &*LIQUIDITY_CACHE_METRICS;
}

/// Metrics manager for tracking price and tick indices.
pub struct IndexCacheMetrics;

impl IndexCacheMetrics {
    const METRIC_INDEX_CACHE_SIZE: &str = "cache_size_index_total";

    /// Initializes and registers descriptions for index-related metrics.
    fn new() -> Self {
        describe_counter!(
            Self::METRIC_INDEX_CACHE_SIZE,
            Unit::Count,
            "Total price indices tracked in cache"
        );

        Self
    }

    /// Increments the total count of registered pools for a specific DEX.
    #[inline]
    pub fn inc(&self, dex: &'static str) {
        let labels = &[(LBL_DEX, dex)];
        counter!(Self::METRIC_INDEX_CACHE_SIZE, labels).increment(1);
    }
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
    pub fn inc(&self, dex: &'static str) {
        let labels = &[(LBL_DEX, dex)];
        counter!(Self::METRIC_POOL_CACHE_SIZE, labels).increment(1);
    }
}

/// Metrics manager for tracking liquidity depth (tick/bin arrays).
pub struct LiquidityCacheMetrics;

impl LiquidityCacheMetrics {
    const METRIC_LIQUIDITY_CACHE_SIZE: &str = "cache_size_liquidity_total";
    const METRIC_LIQUIDITY_DENSITY: &str = "cache_size_liquidity_arrays_per_pool";

    /// Buckets for liquidity density histogram (count of arrays/bins per pool).
    /// Focuses on integer values from 1 to 10, with steps up to 50 for deep caches.
    pub const LIQUIDITY_DENSITY_BUCKETS: &[f64] = &[
        1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 15.0, 20.0, 50.0,
    ];

    /// Initializes and registers descriptions for liquidity-related metrics.
    fn new() -> Self {
        describe_gauge!(
            Self::METRIC_LIQUIDITY_CACHE_SIZE,
            Unit::Count,
            "Current liquidity arrays in memory"
        );
        describe_histogram!(
            Self::METRIC_LIQUIDITY_DENSITY,
            Unit::Count,
            "Distribution of cached liquidity arrays count per single pool"
        );

        HttpMetrics::register_buckets(
            metrics_exporter_prometheus::Matcher::Full(Self::METRIC_LIQUIDITY_DENSITY.to_owned()),
            Self::LIQUIDITY_DENSITY_BUCKETS.to_vec(),
        );

        Self
    }

    /// Sets the absolute number of liquidity arrays currently stored for a specific DEX.
    #[inline]
    pub fn set_liquidity(&self, dex: &'static str, value: usize) {
        let labels = &[(LBL_DEX, dex)];
        gauge!(Self::METRIC_LIQUIDITY_CACHE_SIZE, labels).set(value as f64);
    }

    /// Records the current array count for a specific pool.
    /// Helps analyze how many arrays are typically cached per pool relative to the configured
    /// depth.
    #[inline]
    pub fn record_liquidity_density(&self, dex: &'static str, count: usize) {
        let labels = &[(LBL_DEX, dex)];
        histogram!(Self::METRIC_LIQUIDITY_DENSITY, labels).record(count as f64);
    }
}
