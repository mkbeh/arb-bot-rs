use std::sync::LazyLock;

use metrics::{counter, describe_counter};
use tracing::warn;

use crate::services::enums::ChainStatus;

/// Global metrics registry for the application.
pub static METRICS: LazyLock<Metrics> = LazyLock::new(|| {
    // Describe counters during initialization
    describe_counter!(
        "book_ticker_events_total",
        "Total number of received book ticker events",
    );

    describe_counter!(
        "processed_chains_total",
        "Total number of processed arbitrage chains",
    );

    describe_counter!(
        "profit_orders_total",
        "Total number of profitable orders found",
    );

    Metrics
});

/// Application metrics facade (static methods for incrementing counters)
pub struct Metrics;

impl Metrics {
    /// Increments the book ticker events counter for a specific symbol.
    pub fn add_book_ticker_event(&self, symbol: &str) {
        counter!(
            "book_ticker_events_total",
            "symbol" => symbol.to_string(),
        )
        .increment(1);
    }

    /// Increments the chains counter with labels for symbols and status.
    pub fn add_processed_chain(&self, symbols: &[&str]) {
        if symbols.len() < 3 {
            warn!(
                "Expected at least 3 symbols for chain status metric, got {}",
                symbols.len()
            );
            return;
        }

        // Use first 3 symbols as labels (fixed for metric compatibility)
        counter!(
            "processed_chains_total",
            "symbol_a" => symbols[0].to_string(),
            "symbol_b" => symbols[1].to_string(),
            "symbol_c" => symbols[2].to_string()
        )
        .increment(1);
    }

    /// Increments the chains counter status with labels for symbols and status.
    pub fn add_chain_status(&self, symbols: &[&str], status: ChainStatus) {
        if symbols.len() < 3 {
            warn!(
                "Expected at least 3 symbols for chain status metric, got {}",
                symbols.len()
            );
            return;
        }

        // Use first 3 symbols as labels (fixed for metric compatibility)
        counter!(
            "profit_orders_total",
            "symbol_a" => symbols[0].to_string(),
            "symbol_b" => symbols[1].to_string(),
            "symbol_c" => symbols[2].to_string(),
            "status" => status.to_string(),
        )
        .increment(1);
    }
}
