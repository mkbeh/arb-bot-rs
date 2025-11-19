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
        std::process::exit(1);
    }))
}
