use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

/// A trait for types that require internal consistency checks and parameter initialization.
///
/// This trait is primarily used by configuration structures to ensure that all
/// provided values are within valid ranges before the engine starts.
pub trait Validatable {
    /// Validates the internal state of the object.
    fn validate(&mut self) -> anyhow::Result<()>;
}

#[async_trait]
pub trait ArbitrageService: Send + Sync {
    async fn start(&self, token: CancellationToken) -> anyhow::Result<()>;
}

#[async_trait]
pub trait Exchange: ArbitrageService {}

#[async_trait]
pub trait Sender: ArbitrageService {}
