//! Exponential Backoff Utility
//!
//! This module provides a simple and efficient exponential backoff implementation
//! with automatic reset after a period of success. It is designed for retrying
//! operations that may fail transiently (e.g., network requests, RPC calls,
//! rate-limited APIs, database connections, etc.).
//!
//! # Example
//! ```rust
//! use std::time::Duration;
//!
//! use tools::misc::backoff::ExponentialBackoff;
//!
//! let mut backoff = ExponentialBackoff::new(
//!     Duration::from_millis(100),
//!     Duration::from_secs(60),
//!     Duration::from_secs(30),
//! );
//!
//! let d1 = backoff.next_delay(); // 100ms
//! let d2 = backoff.next_delay(); // 200ms
//! let d3 = backoff.next_delay(); // 400ms
//!
//! assert_eq!(d1, Duration::from_millis(100));
//! assert_eq!(d2, Duration::from_millis(200));
//! assert_eq!(d3, Duration::from_millis(400));
//! ```

use std::time::{Duration, Instant};

/// Exponential backoff strategy with automatic reset after a period of success.
///
/// This implementation increases the delay exponentially (×2) after each failure,
/// caps it at a maximum value, and resets to the initial delay if no failures occur
/// for longer than `reset_threshold`.
///
/// Useful for retrying network requests, API calls, or any operation that can fail
/// transiently (rate limits, temporary outages, etc.).
#[derive(Debug, Clone, Copy)]
pub struct ExponentialBackoff {
    /// Current delay value that will be returned on the next call to `next()`
    current: Duration,

    /// Initial delay value to start with and reset to after successful period
    initial: Duration,

    /// Maximum allowed delay (cap) — won't grow beyond this value
    max: Duration,

    /// Timestamp of the last successful operation (used for reset logic)
    last_success: Instant,

    /// How long without failures must pass before resetting back to `initial`
    reset_threshold: Duration,
}

impl ExponentialBackoff {
    /// Creates a new `ExponentialBackoff`.
    ///
    /// * `initial` — starting delay
    /// * `max` — maximum delay cap
    /// * `reset_threshold` — how long without failures before delay resets to `initial`
    #[must_use]
    pub fn new(initial: Duration, max: Duration, reset_threshold: Duration) -> Self {
        Self {
            current: initial,
            initial,
            max,
            last_success: Instant::now(),
            reset_threshold,
        }
    }

    /// Returns the next delay duration and doubles the current delay for the next call
    /// (capped at `max`).
    ///
    /// This should be called after each **failed** attempt.
    ///
    /// # Returns
    /// The delay to wait before the next retry.
    pub fn next_delay(&mut self) -> Duration {
        let delay = self.current;
        self.current = (self.current * 2).min(self.max);
        delay
    }

    /// Notifies the backoff that an operation **succeeded**.
    ///
    /// This updates the `last_success` timestamp and resets the delay to `initial`
    /// if no failures have occurred for at least `reset_threshold`.
    ///
    /// Should be called after every **successful** operation/retry.
    pub fn reset(&mut self) {
        if self.last_success.elapsed() >= self.reset_threshold {
            self.current = self.initial;
        }
        self.last_success = Instant::now();
    }
}
