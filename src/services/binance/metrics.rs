use std::{
    fmt::{Display, Formatter},
    sync::LazyLock,
};

use metrics::{counter, describe_counter};

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
    pub fn increment_book_ticker_events(&self, symbol: &str) {
        counter!(
            "book_ticker_events_total",
            "symbol" => symbol.to_string(),
        )
        .increment(1);
    }

    pub fn increment_processed_chains(&self, symbols: &[&str]) {
        counter!(
            "processed_chains_total",
            "symbol_a" => symbols[0].to_string(),
            "symbol_b" => symbols[1].to_string(),
            "symbol_c" => symbols[2].to_string()
        )
        .increment(1);
    }

    pub fn increment_profit_orders(&self, symbols: &[&str], status: ProcessChainStatus) {
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

pub enum ProcessChainStatus {
    New,
    Filled,
    Cancelled,
}

impl Display for ProcessChainStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ProcessChainStatus::New => write!(f, "new"),
            ProcessChainStatus::Filled => write!(f, "filled"),
            ProcessChainStatus::Cancelled => write!(f, "cancelled"),
        }
    }
}
