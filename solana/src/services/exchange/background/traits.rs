use std::time::Duration;

use async_trait::async_trait;
use tokio_util::sync::CancellationToken;
use tools::misc::backoff::ExponentialBackoff;
use tracing::error;

#[async_trait]
pub trait BackgroundService {
    fn execute_interval(&self) -> Duration;
    async fn execute(&self) -> anyhow::Result<()>;

    async fn start(&self, token: CancellationToken) -> anyhow::Result<()> {
        let mut interval = tokio::time::interval(self.execute_interval());

        let mut backoff = ExponentialBackoff::new(
            Duration::from_secs(1),
            Duration::from_secs(60),
            Duration::from_secs(30),
        );

        loop {
            tokio::select! {
                _ = token.cancelled() => break,
                _ = interval.tick() => {
                    match self.execute().await {
                        Ok(()) => {
                            backoff.reset();
                        }
                        Err(e) => {
                            let delay = backoff.next_delay();
                            error!(
                                "[{}] Failed to execute: {e:#?}. Retrying in {delay:?}...",
                                std::any::type_name::<Self>()
                            );

                            tokio::select! {
                                _ = token.cancelled() => break,
                                _ = tokio::time::sleep(delay) => {}
                            }

                            interval.reset();
                        }
                    }
                }
            }
        }

        Ok(())
    }
}
