use std::{sync::OnceLock, time};

use async_trait::async_trait;
use tokio_util::sync::CancellationToken;
use tracing::info;

use crate::libs::http_server::server::ServerProcess;

pub struct Config {
    pub delay: u64,
}

pub struct Process {
    delay: u64,
}

impl Process {
    pub fn new(cfg: Config) -> &'static Self {
        static INSTANCE: OnceLock<Process> = OnceLock::new();
        INSTANCE.get_or_init(|| Process { delay: cfg.delay })
    }
}

#[async_trait]
impl ServerProcess for Process {
    async fn pre_run(&self) -> anyhow::Result<()> {
        info!("Starting process");
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
                    info!("process delayed");
            }
            }
        }
    }
}
