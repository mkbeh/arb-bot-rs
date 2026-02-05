use std::time::{SystemTime, UNIX_EPOCH};

/// Computes the current timestamp in milliseconds since the Unix epoch.
#[must_use]
#[inline(always)]
pub fn get_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_millis() as u64
}
