pub mod amm_config;
pub mod index;
pub mod liquidity;
pub mod metrics;
pub mod mint;
mod oracle;
pub mod pool;
pub mod state;
pub mod vault;

pub use amm_config::*;
pub use index::*;
pub use liquidity::*;
pub use metrics::*;
pub use mint::*;
pub use oracle::*;
pub use pool::*;
pub use state::*;
pub use vault::*;

/// Initializes the global market state and cache metrics.
///
/// Must be called once at application startup before any cache access.
pub fn init(depth: i64) -> anyhow::Result<()> {
    init_market_state(depth)?;
    init_metrics();
    Ok(())
}
