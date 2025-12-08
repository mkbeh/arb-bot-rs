//! Process module for managing the order sender execution loop.
use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use tokio_util::sync::CancellationToken;
use tracing::error;

use crate::{libs::http_server::ServerProcess, services::Sender};

/// Configuration for the order sender process.
pub struct Config {
    pub error_timeout_secs: u64,
}

impl Config {
    pub fn new(error_timeout_secs: u64) -> Self {
        Self { error_timeout_secs }
    }
}

/// Core process for executing order sending operations via an order sender service.
pub struct Process {
    error_timeout_secs: Duration,
    service: Arc<dyn Sender>,
}

impl Process {
    pub fn create(cfg: Config, service: Arc<dyn Sender>) -> Arc<dyn ServerProcess> {
        Arc::new(Self {
            error_timeout_secs: Duration::from_secs(cfg.error_timeout_secs),
            service,
        })
    }
}

/// Implementation of the `ServerProcess` trait for the `Process` struct.
#[async_trait]
impl ServerProcess for Process {
    /// Pre-run hook: Performs any necessary setup before the main loop starts.
    async fn pre_run(&self) -> anyhow::Result<()> {
        Ok(())
    }

    /// Main run loop for the process.
    async fn run(&self, token: CancellationToken) -> anyhow::Result<()> {
        loop {
            tokio::select! {
                _ = token.cancelled() => {
                    break;
                }
                result = self.service.send_orders(token.child_token()) => {
                    if let Err(e) = result {
                        error!(error = ?e, "error during sender process");
                        tokio::time::sleep(self.error_timeout_secs).await;
                    }
                }
            }
        }
        Ok(())
    }
}
