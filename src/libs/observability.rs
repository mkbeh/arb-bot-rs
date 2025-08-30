use std::env;

use tracing_subscriber::{
    EnvFilter, Layer, fmt::format::FmtSpan, layer::SubscriberExt, util::SubscriberInitExt,
};

pub fn setup_opentelemetry(name: &'static str) {
    // Create a new tracing::Fmt layer to print the logs to stdout. It has a
    // default filter of `info` level and above, and `debug` and above for logs
    // from OpenTelemetry crates. The filter levels can be customized as needed.
    let fmt_log_level = env::var("RUST_LOG").unwrap_or_else(|_| "debug".to_owned());

    let filter_fmt = EnvFilter::new(fmt_log_level.clone())
        .add_directive(format!("{name}={fmt_log_level}").parse().unwrap())
        .add_directive("reqwest=error".parse().unwrap())
        .add_directive("tracing=error".parse().unwrap());
    let fmt_layer = tracing_subscriber::fmt::layer()
        .compact()
        .with_span_events(FmtSpan::CLOSE)
        .with_thread_names(true)
        .json()
        .flatten_event(true)
        .with_level(true)
        .with_line_number(true)
        .with_filter(filter_fmt);

    // Initialize the tracing subscriber with the OpenTelemetry layer and the
    // Fmt layer.
    tracing_subscriber::registry().with(fmt_layer).init();
}
