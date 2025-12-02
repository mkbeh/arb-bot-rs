pub mod binance_api;
pub mod hooks;
pub mod http_server;
pub mod kucoin_api;
pub mod macros;
pub mod misc;
pub mod observability;
pub mod toml;

/// Initializes the application with essential setup routines.
///
/// This function should be called early in the application lifecycle
/// to configure panic handling and observability (tracing and OpenTelemetry).
///
/// # Arguments
///
/// * `name` - The name of the application (typically `env!("CARGO_PKG_NAME")`).
///
/// # Example
///
/// ```rust
/// use arb_bot_rs::libs::setup_application;
///
/// setup_application(env!("CARGO_PKG_NAME"));
/// ```
///
/// # Panics
///
/// This function does not panic directly, but the called subroutines
/// (`hooks::setup_panic_hook` and `observability::setup_opentelemetry`)
/// may panic if setup fails (e.g., due to invalid configuration).
pub fn setup_application(name: &'static str) {
    // Setup custom panic hook to handle runtime panics gracefully.
    hooks::setup_panic_hook();

    // Setup logs/tracing with OpenTelemetry integration.
    observability::setup_opentelemetry(name);
}
