use std::sync::{LazyLock, Mutex};

use metrics_exporter_prometheus::{Matcher, PrometheusBuilder, PrometheusHandle};

/// Default buckets for latency-based metrics (seconds).
/// Covers range from 0.1ms to 1s.
pub const DEFAULT_LATENCY_BUCKETS: &[f64] = &[
    0.0001, 0.0005, 0.001, 0.0025, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0,
];

/// Global storage for metric configuration before the recorder is installed.
pub static HTTP_METRICS: LazyLock<Mutex<HttpMetrics>> =
    LazyLock::new(|| Mutex::new(HttpMetrics::default()));

/// Configuration for Prometheus metrics recorder.
/// Allows customizing buckets and other parameters during initialization.
pub struct HttpMetrics {
    /// Overrides for histogram buckets based on metric name patterns.
    pub bucket_overrides: Vec<(Matcher, Vec<f64>)>,
}

impl Default for HttpMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl HttpMetrics {
    #[must_use]
    pub fn new() -> Self {
        Self {
            bucket_overrides: Vec::new(),
        }
    }

    /// Registers custom buckets for a specific matcher (Full, Prefix, or Suffix).
    /// These will be used to render true Prometheus histograms instead of summaries.
    pub fn register_buckets(matcher: Matcher, buckets: Vec<f64>) {
        if let Ok(mut cfg) = HTTP_METRICS.lock() {
            cfg.bucket_overrides.push((matcher, buckets));
        }
    }
}

/// Installs and returns a Prometheus metrics recorder handle.
///
/// Panics if installation fails (e.g., duplicate recorder).
pub fn setup_metrics_recorder() -> PrometheusHandle {
    let mut builder = PrometheusBuilder::new()
        .set_buckets(DEFAULT_LATENCY_BUCKETS)
        .expect("Failed to set default buckets");

    // Apply custom overrides from global config
    if let Ok(cfg) = HTTP_METRICS.lock() {
        for (matcher, buckets) in &cfg.bucket_overrides {
            builder = builder
                .set_buckets_for_metric(matcher.clone(), buckets)
                .expect("Failed to set custom buckets");
        }
    }

    builder
        .install_recorder()
        .expect("Failed to install recorder")
}
