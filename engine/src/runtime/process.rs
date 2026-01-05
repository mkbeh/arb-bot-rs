use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use tokio_util::sync::CancellationToken;
use tools::http::http_server::HttpServerProcess;
use tracing::error;

use crate::service::traits::ArbitrageService;

pub struct GenericProcess<S>
where
    S: ArbitrageService + ?Sized,
{
    error_timeout_secs: Duration,
    service: Arc<S>,
}

impl<S: ArbitrageService + ?Sized + 'static> GenericProcess<S> {
    pub fn new(service: Arc<S>) -> Self {
        Self {
            service,
            error_timeout_secs: Duration::from_secs(60),
        }
    }
}

#[async_trait]
impl<S: ArbitrageService + ?Sized + 'static> HttpServerProcess for GenericProcess<S> {
    async fn pre_run(&self) -> anyhow::Result<()> {
        Ok(())
    }

    async fn run(&self, token: CancellationToken) -> anyhow::Result<()> {
        loop {
            tokio::select! {
                _ = token.cancelled() => break,
                result = self.service.start(token.child_token()) => {
                    if let Err(e) = result {
                        error!(error = ?e, "error during arbitrage process");
                        tokio::time::sleep(self.error_timeout_secs).await;
                    }
                }
            }
        }
        Ok(())
    }
}
