use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

use crate::libs::solana_client::callback::BatchEventCallbackWrapper;

/// Trait defining the behavior for Solana data streams (gRPC, WebSocket, etc.).
/// Handles event processing through callbacks and manages the subscription lifecycle.
#[async_trait]
pub trait SolanaStream: Send + Sync {
    /// Attaches a callback function that is triggered whenever a batch of events is received.
    ///
    /// This method follows the Builder pattern, allowing for fluent initialization.
    ///
    /// # Arguments
    /// * `callback` - A thread-safe closure or function that processes a `Vec<Event>`. It must
    ///   return `anyhow::Result<()>`. If it returns an `Err`, the stream may terminate.
    fn set_callback(&mut self, callback: BatchEventCallbackWrapper);

    /// Starts the main subscription loop with an automatic retry mechanism.
    ///
    /// The implementation should handle network drops and perform exponential backoff
    /// during reconnection attempts.
    ///
    /// # Errors
    /// Returns an error if the initial connection fails or if the configuration is invalid.
    async fn subscribe(&mut self, token: CancellationToken) -> anyhow::Result<()>;
}
