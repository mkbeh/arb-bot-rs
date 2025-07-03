pub mod binance_api;
pub mod closer;
pub mod hooks;
pub mod http_server;
pub mod observability;
pub mod toml;
pub mod utils;

pub fn setup_application(name: &'static str) {
    // Setup custom panic hook
    hooks::setup_panic_hook();

    // Setup logs/tracing
    observability::setup_opentelemetry(name);
}
