pub mod binance_api;
pub mod hooks;
pub mod http_server;
mod kucoin_api;
pub mod misc;
pub mod observability;
pub mod toml;

pub fn setup_application(name: &'static str) {
    // Setup custom panic hook
    hooks::setup_panic_hook();

    // Setup logs/tracing
    observability::setup_opentelemetry(name);
}
