pub mod amm_config;
pub mod bitmap;
pub mod liquidity;
mod metrics;
pub mod mint;
pub mod oracle;
pub mod pool;
pub mod reserve;
pub mod state;
pub mod sync;
pub mod vault;

pub use amm_config::*;
pub use bitmap::*;
pub use liquidity::*;
pub use metrics::*;
pub use mint::*;
pub use oracle::*;
pub use pool::*;
pub use reserve::*;
pub use state::*;
pub use sync::*;
pub use vault::*;

/// Initializes the global market state and cache metrics.
///
/// Must be called once at application startup before any cache access.
pub fn init_local_cache() -> anyhow::Result<()> {
    init_market_state()?;
    init_cache_metrics();
    Ok(())
}
