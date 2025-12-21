/// Macro for setting up the main function of the application.
///
/// This macro generates an asynchronous inner function `__inner_main`, annotated with
/// `#[tokio::main]`, which initializes the application using `setup_application` (with the crate
/// name from `CARGO_PKG_NAME`), executes the provided async code block, and returns
/// `anyhow::Result<()>`.
///
/// The synchronous `main` function calls `__inner_main`, logs errors using `tracing::error!`,
/// and exits the process with code 1 in case of an error.
///
/// # Usage
///
/// ```rust,no_run
/// #[macro_use]
/// extern crate arb_bot_rs;
/// use anyhow::Result;
///
/// setup_app!(async {
///     // Your async code here
///     tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
///     println!("Application started!");
///     Ok(())
/// });
/// ```
///
/// # Notes
/// - The macro is exported (`#[macro_export]`), so it can be used in other crates.
/// - The provided block `$block` must return `impl Future<Output = anyhow::Result<()>>`.
#[macro_export]
macro_rules! setup_app {
    // Pattern: accepts a single async block (expression).
    ($block:expr) => {
        use anyhow::Context;

        // Generate an async inner main function with Tokio runtime.
        #[::tokio::main]
        async fn __inner_main() -> anyhow::Result<()> {
            // Application initialization
            $crate::libs::setup_application(env!("CARGO_PKG_NAME"))
                .context("Failed to setup application")?;

            // Execute the provided async block and await its completion.
            $block.await
        }

        // Synchronous entry point for the program.
        fn main() {
            if let Err(e) = __inner_main() {
                tracing::error!("{:?}", e);
                std::process::exit(1);
            }
        }
    };
}
