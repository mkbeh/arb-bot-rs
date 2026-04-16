use std::{sync::LazyLock, time::Instant};

use metrics::{Unit, counter, describe_counter, describe_histogram, histogram};

/// Global access point for stream-related metrics.
pub static STREAM_METRICS: LazyLock<Metrics> = LazyLock::new(Metrics::default);

/// Metrics handle providing methods for recording various system events.
pub struct Metrics;

impl Default for Metrics {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(clippy::unused_self)]
impl Metrics {
    // Labels
    const LBL_TRANSPORT: &'static str = "transport";
    const LBL_TYPE: &'static str = "type";
    const LBL_PROGRAM: &'static str = "program_id";
    const LBL_ERROR: &'static str = "error";

    // Metric Names
    const METRIC_BYTES: &'static str = "solana_client_bytes_total";
    const METRIC_EVENTS: &'static str = "solana_client_events_total";
    const METRIC_ERRORS: &'static str = "solana_client_errors_total";
    const METRIC_LATENCY: &'static str = "solana_client_processing_duration_seconds";
    const METRIC_HANDLER: &'static str = "solana_client_handler_duration_seconds";
    const METRIC_BATCH_SIZE: &'static str = "solana_client_stream_batch_size";

    /// Buckets for incoming message batch sizes.
    /// Covers small updates (1-100) and heavy spikes up to 10k messages.
    pub const STREAM_BATCH_SIZE_BUCKETS: &[f64] = &[
        // Active workload zone: High granularity for typical batch sizes (1 to 500).
        1.0, 10.0, 25.0, 50.0, 75.0, 100.0, 150.0, 200.0, 250.0, 350.0, 500.0,
        // Scalability zone: Moderate granularity for increased throughput and load growth.
        750.0, 1000.0, 2000.0, 3500.0, 5000.0,
        // Extreme spike zone: Low granularity for capturing anomalous bursts up to 10k.
        7500.0, 10000.0,
    ];

    #[must_use]
    pub fn new() -> Self {
        describe_counter!(
            Self::METRIC_BYTES,
            Unit::Bytes,
            "Total data throughput by transport and program"
        );
        describe_counter!(
            Self::METRIC_EVENTS,
            Unit::Count,
            "Successfully parsed events"
        );
        describe_counter!(Self::METRIC_ERRORS, Unit::Count, "Total errors by type");
        describe_histogram!(Self::METRIC_LATENCY, Unit::Seconds, "Processing latency");
        describe_histogram!(
            Self::METRIC_HANDLER,
            Unit::Seconds,
            "Time taken by the callback/handler to process events"
        );
        describe_histogram!(
            Self::METRIC_BATCH_SIZE,
            Unit::Count,
            "Distribution of incoming message batch sizes before parsing"
        );

        Self
    }

    /// Returns buckets and metric name for external registration.
    #[must_use]
    pub fn batch_size_buckets() -> (&'static str, &'static [f64]) {
        (Self::METRIC_BATCH_SIZE, Self::STREAM_BATCH_SIZE_BUCKETS)
    }

    // === Transport Layer (WS / gRPC / RPC) ===

    /// Records the number of bytes received over the wire.
    pub fn record_bytes(
        &self,
        transport: Transport,
        event_type: EventType,
        program_id: &'static str,
        len: usize,
    ) {
        let labels = [
            (Self::LBL_TRANSPORT, transport.as_str()),
            (Self::LBL_TYPE, event_type.as_str()),
            (Self::LBL_PROGRAM, program_id),
        ];
        counter!(Self::METRIC_BYTES, &labels).increment(len as u64);
    }

    // === Parsing Layer ===

    /// Increments the successful event counter when a message is fully parsed.
    pub fn record_event(
        &self,
        transport: Transport,
        event_type: EventType,
        program_id: &'static str,
    ) {
        let labels = [
            (Self::LBL_TRANSPORT, transport.as_str()),
            (Self::LBL_TYPE, event_type.as_str()),
            (Self::LBL_PROGRAM, program_id),
        ];
        counter!(Self::METRIC_EVENTS, &labels).increment(1);
    }

    /// Records a parsing failure for a specific DEX.
    /// Essential for identifying breaking changes in on-chain program formats.
    pub fn record_error(&self, transport: Transport, kind: StreamErrorKind) {
        let labels = [
            (Self::LBL_TRANSPORT, transport.as_str()),
            (Self::LBL_ERROR, kind.as_str()),
        ];
        counter!(Self::METRIC_ERRORS, &labels).increment(1);
    }

    // === Performance ===

    /// Measures the total time spent in the processing pipeline.
    pub fn record_duration(&self, transport: Transport, start: Instant) {
        let labels = [(Self::LBL_TRANSPORT, transport.as_str())];
        histogram!(Self::METRIC_LATENCY, &labels).record(start.elapsed().as_secs_f64());
    }

    /// Measures the execution time of the arbitrage/business logic.
    pub fn record_handler_duration(&self, start: Instant) {
        histogram!(Self::METRIC_HANDLER).record(start.elapsed().as_secs_f64());
    }

    /// Records the number of messages in a single processed batch.
    pub fn record_batch_size(&self, size: usize) {
        histogram!(Self::METRIC_BATCH_SIZE).record(size as f64);
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
    Program,
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
            Self::Program => "program",
            Self::Tx => "tx",
        }
    }
}

/// Error types for the Solana client.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StreamErrorKind {
    Session,
    Parse,
}

impl StreamErrorKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Session => "session",
            Self::Parse => "parse",
        }
    }
}
