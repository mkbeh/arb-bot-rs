use std::sync::LazyLock;

use metrics::{counter, describe_counter};

use crate::services::enums::ChainStatus;

pub static METRICS: LazyLock<Metrics> = LazyLock::new(|| {
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

pub struct Metrics;

impl Metrics {
    pub fn add_book_ticker_event(&self, symbol: &str) {
        counter!(
            "book_ticker_events_total",
            "symbol" => symbol.to_string(),
        )
        .increment(1);
    }

    pub fn add_processed_chain(&self, symbols: &[&str]) {
        counter!(
            "processed_chains_total",
            "symbol_a" => symbols[0].to_string(),
            "symbol_b" => symbols[1].to_string(),
            "symbol_c" => symbols[2].to_string()
        )
        .increment(1);
    }

    pub fn add_chain_status(&self, symbols: &[&str], status: ChainStatus) {
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
