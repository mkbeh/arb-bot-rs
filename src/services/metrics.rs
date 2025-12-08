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
    pub fn record_book_ticker_event(&self, symbol: &str) {
        counter!(
            "book_ticker_events_total",
            "symbol" => symbol.to_string(),
        )
        .increment(1);
    }

    /// Increments the chains counter with labels for symbols and status.
    pub fn record_processed_chain(&self, symbols: &[&str]) {
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
    pub fn record_chain_status(&self, symbols: &[&str], status: ChainStatus) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::enums::ChainStatus;

    #[test]
    fn test_record_book_ticker_event() {
        // Smoke test: no panic on call
        Metrics.record_book_ticker_event("BTCUSDT");
    }

    #[test]
    fn test_record_processed_chain_valid() {
        // Smoke test: no panic with 3+ symbols
        let symbols = vec!["BTCUSDT", "ETHUSDT", "ADAUSDT"];
        Metrics.record_processed_chain(&symbols);
    }

    #[test]
    fn test_record_processed_chain_invalid_short() {
        // Smoke test: no panic with short symbols (warn logged, but test passes if no panic)
        let symbols = vec!["BTCUSDT"];
        Metrics.record_processed_chain(&symbols);
    }

    #[test]
    fn test_record_chain_status_valid() {
        // Smoke test: no panic with 3+ symbols and status
        let symbols = vec!["BTCUSDT", "ETHUSDT", "ADAUSDT"];
        let status = ChainStatus::Filled;
        Metrics.record_chain_status(&symbols, status);
    }

    #[test]
    fn test_record_chain_status_invalid_short() {
        // Smoke test: no panic with short symbols (warn logged)
        let symbols = vec!["BTCUSDT"];
        let status = ChainStatus::New;
        Metrics.record_chain_status(&symbols, status);
    }

    #[test]
    fn test_record_chain_status_different_status() {
        // Smoke test: multiple calls with different statuses
        let symbols = vec!["BTCUSDT", "ETHUSDT", "ADAUSDT"];

        Metrics.record_chain_status(&symbols, ChainStatus::New);
        Metrics.record_chain_status(&symbols, ChainStatus::Filled);
        Metrics.record_chain_status(&symbols, ChainStatus::Cancelled);
    }
}
