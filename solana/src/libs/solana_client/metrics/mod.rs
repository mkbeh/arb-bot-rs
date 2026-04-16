pub mod labels;
pub mod stream;

pub use labels::*;
pub use stream::*;

/// Returns all histogram bucket configurations for external registration.
///
/// Each entry is `(metric_name, buckets)`.
#[must_use]
pub fn histogram_buckets() -> Vec<(&'static str, &'static [f64])> {
    vec![Metrics::batch_size_buckets()]
}
