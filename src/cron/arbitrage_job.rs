use std::{
    sync::{Arc, OnceLock},
    time,
};

use async_trait::async_trait;
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

use crate::{libs::http_server::server::ServerProcess, services::ExchangeService};

pub struct Config {
    pub delay: u64,
}

pub struct Process {
    delay: u64,
    service: Arc<dyn ExchangeService>,
}

impl Process {
    pub fn new(cfg: Config, service: Arc<dyn ExchangeService>) -> &'static Self {
        static INSTANCE: OnceLock<Process> = OnceLock::new();
        INSTANCE.get_or_init(|| Process {
            delay: cfg.delay,
            service,
        })
    }
}

#[async_trait]
impl ServerProcess for Process {
    async fn pre_run(&self) -> anyhow::Result<()> {
        info!("Pre running process");
        Ok(())
    }

    async fn run(&self, token: CancellationToken) -> anyhow::Result<()> {
        loop {
            tokio::select! {
                _ = token.cancelled() => {
                info!("process successfully stopped");
                return Ok(());
            }
            _ = tokio::time::sleep(time::Duration::from_secs(self.delay)) => {
                    match self.service.start_arbitrage().await {
                        Ok(_) => info!(count = "process complete successfully"),
                        Err(e) => error!(error = ?e, "error during working process"),
                    };
            }
            }
        }
    }
}
