use std::{sync::LazyLock, time::Instant};

use metrics::{counter, describe_counter, describe_histogram, histogram};

// --- Label Keys ---
const LBL_TRANSPORT: &str = "transport";
const LBL_TYPE: &str = "type";
const LBL_PROGRAM: &str = "program_id";

/// Global access point for stream-related metrics.
pub static STREAM_METRICS: LazyLock<Metrics> = LazyLock::new(|| {
    // --- Message Flows ---
    describe_counter!(
        "solana_client_bytes_total",
        "Total data throughput by transport and program"
    );

    // --- Events & Parsing ---
    describe_counter!("solana_client_events_total", "Successfully parsed events");

    describe_counter!(
        "solana_client_parse_errors_total",
        "Failed parsing attempts"
    );

    // --- Latency ---
    describe_histogram!(
        "solana_client_processing_duration_seconds",
        "Processing latency"
    );

    describe_histogram!(
        "solana_client_handler_duration_seconds",
        "Time taken by the callback/handler to process events"
    );

    Metrics
});

/// Metrics handle providing methods for recording various system events.
pub struct Metrics;

#[allow(clippy::unused_self)]
impl Metrics {
    // === Transport Layer (WS / gRPC / RPC) ===

    /// Records the number of bytes received over the wire.
    pub fn observe_bytes(
        &self,
        transport: Transport,
        event_type: EventType,
        program_id: &'static str,
        len: usize,
    ) {
        let labels = [
            (LBL_TRANSPORT, transport.as_str()),
            (LBL_TYPE, event_type.as_str()),
            (LBL_PROGRAM, program_id),
        ];
        counter!("solana_client_bytes_total", &labels).increment(len as u64);
    }

    // === Parsing Layer ===

    /// Increments the successful event counter when a message is fully parsed.
    pub fn inc_events(
        &self,
        transport: Transport,
        event_type: EventType,
        program_id: &'static str,
    ) {
        let labels = [
            (LBL_TRANSPORT, transport.as_str()),
            (LBL_TYPE, event_type.as_str()),
            (LBL_PROGRAM, program_id),
        ];
        counter!("solana_client_events_total", &labels).increment(1);
    }

    /// Records a parsing failure for a specific DEX.
    /// Essential for identifying breaking changes in on-chain program formats.
    pub fn inc_parse_error(&self, transport: Transport, program_id: &'static str) {
        let labels = [
            (LBL_TRANSPORT, transport.as_str()),
            (LBL_PROGRAM, program_id),
        ];
        counter!("solana_client_parse_errors_total", &labels).increment(1);
    }

    // === Performance ===

    /// Measures the total time spent in the processing pipeline.
    pub fn observe_duration(&self, transport: Transport, start: Instant) {
        let labels = [(LBL_TRANSPORT, transport.as_str())];
        histogram!("solana_client_processing_duration_seconds", &labels)
            .record(start.elapsed().as_secs_f64());
    }

    /// Measures the execution time of the arbitrage/business logic.
    pub fn observe_handler_duration(&self, start: Instant) {
        histogram!("solana_client_handler_duration_seconds").record(start.elapsed().as_secs_f64());
    }
}

/// Supported transport layers for Solana client connections.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Transport {
    Ws,
    Grpc,
}

/// Categorization of events received from the blockchain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventType {
    Slot,
    Account,
    Tx, // For logs/transactions
}

impl Transport {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ws => "ws",
            Self::Grpc => "grpc",
        }
    }
}

impl EventType {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Slot => "slot",
            Self::Account => "account",
            Self::Tx => "tx",
        }
    }
}
