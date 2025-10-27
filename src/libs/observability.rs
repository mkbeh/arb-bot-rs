use std::env;

use tracing_subscriber::{
    EnvFilter, fmt::format::FmtSpan, layer::SubscriberExt, prelude::*, util::SubscriberInitExt,
};

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

    tracing_subscriber::registry().with(fmt_layer).init();
}
