use std::{
    sync::{Arc, OnceLock},
    time::Duration,
};

use async_trait::async_trait;
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;
use tracing::error;

use crate::{libs::http_server::ServerProcess, services::ExchangeService};

pub struct Config {
    pub timeout_secs: u64,
    pub error_timeout_secs: u64,
}

impl Config {
    pub fn new(timeout_secs: u64, error_timeout_secs: u64) -> Self {
        Self {
            timeout_secs,
            error_timeout_secs,
        }
    }
}

pub struct Process {
    timeout_secs: Duration,
    error_timeout_secs: Duration,
    service: Arc<dyn ExchangeService>,
}

impl Process {
    pub fn new(cfg: Config, service: Arc<dyn ExchangeService>) -> &'static Self {
        static INSTANCE: OnceLock<Process> = OnceLock::new();
        INSTANCE.get_or_init(|| Process {
            timeout_secs: Duration::from_secs(cfg.timeout_secs),
            error_timeout_secs: Duration::from_secs(cfg.error_timeout_secs),
            service,
        })
    }
}

#[async_trait]
impl ServerProcess for Process {
    async fn pre_run(&self) -> anyhow::Result<()> {
        Ok(())
    }

    async fn run(&self, token: CancellationToken) -> anyhow::Result<()> {
        loop {
            tokio::select! {
            _ = token.cancelled() => {
                return Ok(());
            }
            _ = tokio::time::sleep(self.timeout_secs) => {
                if let Err(e) = self.service.start_arbitrage().await {
                    error!(error = ?e, "error during arbitrage process");
                    sleep(self.error_timeout_secs).await;
                }
            }
            }
        }
    }
}
