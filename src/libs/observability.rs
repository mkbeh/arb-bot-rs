use std::env;

use tracing_subscriber::{
    EnvFilter, fmt::format::FmtSpan, layer::SubscriberExt, prelude::*, util::SubscriberInitExt,
};

/// Sets up tracing for the application using `tracing_subscriber`.
///
/// This function initializes a tracing subscriber with a formatted layer based on the `RUST_LOG`
/// environment variable (defaults to "debug" if not set). It configures a filter to control log
/// levels for the application (using the provided `name` as the crate/module prefix) and suppresses
/// noisy logs from common dependencies like `hyper`, `reqwest`, etc., while enabling specific
/// traces where useful (e.g., Axum rejections).
///
/// The output is formatted with ANSI colors, without file/line info for brevity, and written to
/// stdout. Span events are disabled to reduce verbosity.
///
/// # Arguments
/// * `name` - Static string representing the application or crate name (e.g., "my_app") for
///   targeted logging.
///
/// # Panics
/// Panics if parsing the log level directives fails (unlikely, as they are hardcoded).
///
/// # Usage
/// Call this early in `main()` to initialize logging before any traced operations.
/// Example: `setup_opentelemetry("arbitrage_bot");`
/// Requires the `tracing` and `tracing-subscriber` crates.
pub fn setup_opentelemetry(name: &'static str) {
    let fmt_log_level = env::var("RUST_LOG").unwrap_or_else(|_| "debug".to_owned());

    let filter_fmt = EnvFilter::new(fmt_log_level.clone())
        .add_directive(format!("{name}={fmt_log_level}").parse().unwrap())
        .add_directive("hyper=error".parse().unwrap())
        .add_directive("h2=error".parse().unwrap())
        .add_directive("reqwest=error".parse().unwrap())
        .add_directive("tower_http=error".parse().unwrap())
        .add_directive("axum::rejection=trace".parse().unwrap())
        .add_directive("tungstenite=info".parse().unwrap())
        .add_directive("tracing=error".parse().unwrap());

    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_span_events(FmtSpan::NONE)
        .with_level(true)
        .with_target(false)
        .with_line_number(false)
        .with_file(false)
        .with_ansi(true)
        .with_writer(std::io::stdout)
        .with_filter(filter_fmt);

    // Initialize the global subscriber with the formatted layer.
    tracing_subscriber::registry().with(fmt_layer).init();
}
