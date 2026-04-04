use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[must_use]
#[inline(always)]
pub fn get_timestamp() -> Duration {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
}

#[must_use]
#[inline(always)]
pub fn get_timestamp_ms() -> u64 {
    get_timestamp().as_millis() as u64
}
