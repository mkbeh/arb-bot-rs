use std::time::Duration;

/// Returns the current timestamp as a `Duration` since the UNIX epoch (January 1, 1970, 00:00:00
/// UTC).
///
/// This function uses `SystemTime::now()` to fetch the current system time and computes the
/// duration since the UNIX epoch. It panics if the system time is before the epoch (e.g., "time
/// went backwards"), which is an unlikely but possible edge case on some systems.
///
/// # Platform Availability
/// This function is only available on non-WebAssembly (non-WASM) targets that are not Emscripten or
/// WASI. For WASM targets, it is a no-op (function not compiled).
///
/// # Returns
/// A `Duration` representing milliseconds since the UNIX epoch.
///
/// # Panics
/// Panics if the current system time is before the UNIX epoch.
///
/// # Examples
/// ```
/// use std::time::Duration;
///
/// use arb_bot_rs::libs::misc::time::get_current_timestamp;
///
/// let now = get_current_timestamp();
/// println!("Current timestamp: {:?}", now);
/// ```
///
/// # Notes
/// Marked as `#[must_use]` to encourage handling the returned value.
/// Use `std::time::SystemTime` directly for more flexible time operations.
#[cfg(not(all(
    target_arch = "wasm32",
    not(any(target_os = "emscripten", target_os = "wasi"))
)))]
#[must_use]
pub fn get_current_timestamp() -> Duration {
    let start = std::time::SystemTime::now();
    start
        .duration_since(std::time::UNIX_EPOCH)
        .expect("Time went backwards")
}
