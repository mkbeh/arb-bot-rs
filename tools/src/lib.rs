pub mod http;
pub mod misc;
pub mod observability;
pub mod toml;

use rustls::crypto::ring;
#[cfg(feature = "derive")]
pub use tools_derive::main;

use crate::observability::setup_opentelemetry;

/// Initializes the application with essential setup routines.
///
/// This function should be called early in the application lifecycle
/// to configure panic handling, observability (tracing and OpenTelemetry),
/// and TLS crypto provider.
///
/// # Arguments
///
/// * `name` - The name of the application (typically `env!("CARGO_PKG_NAME")`).
///
/// # Errors
///
/// Returns an error if setup fails (e.g., tracing init or rustls provider install).
///
/// # Example
///
/// ```rust
/// use tools::setup_application;
///
/// setup_application(env!("CARGO_PKG_NAME")).expect("Setup failed");
/// ```
///
/// # Panics
///
/// This function does not panic directly, but subroutines may if configuration is invalid.
pub fn setup_application(name: &'static str) -> anyhow::Result<()> {
    // Setup custom panic hook to handle runtime panics gracefully.
    setup_panic_hook();
    // Setup logs/tracing with OpenTelemetry integration.
    setup_opentelemetry(name);
    // Install rustls crypto provider (ring backend) to fix TLS init panic.
    setup_tls_provider()
}

/// Sets up a custom panic hook for the application.
///
/// This function configures Rust's panic handler to log panic information using the `tracing`
/// crate. It captures the panic message and, if available, the source location (file, line, column)
/// as structured fields. After logging, the process exits with code 1.
///
/// # Panics
/// This function does not panic itself, but it overrides the default panic behavior.
///
/// # Usage
/// Call this function early in `main()` to ensure all panics are logged properly.
/// Requires the `tracing` crate for logging.
///
/// # Example
///
/// ```rust
/// use tools::setup_panic_hook;
///
/// setup_panic_hook();
/// ```
pub fn setup_panic_hook() {
    std::panic::set_hook(Box::new(move |panic_info| {
        // If the panic has a source location, record it as structured fields.
        if let Some(location) = panic_info.location() {
            tracing::error!(
                message = %panic_info,
                panic.file = location.file(),
                panic.line = location.line(),
                panic.column = location.column(),
            );
        } else {
            tracing::error!(message = %panic_info);
        }

        #[allow(clippy::exit)]
        std::process::exit(1);
    }))
}

/// Installs the rustls crypto provider (ring backend) to fix TLS init panic.
///
/// This function is called early in the application lifecycle (e.g., in main before gRPC connect)
/// to explicitly select ring as the crypto provider for rustls. In rustls 0.23+, the provider is
/// not automatically determined, leading to a panic during TLS handshake (e.g., "Could not
/// automatically determine the process-level CryptoProvider").
///
/// # Example
///
/// ```rust
/// use tools::setup_tls_provider;
///
/// setup_tls_provider();
/// ```
pub fn setup_tls_provider() -> anyhow::Result<()> {
    ring::default_provider()
        .install_default()
        .map_err(|e| anyhow::anyhow!("Failed to install rustls crypto provider: {e:?}"))
}
