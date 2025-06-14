use std::{sync::OnceLock, time};

use async_trait::async_trait;
use tokio_util::sync::CancellationToken;
use tracing::info;

use crate::libs::http_server::server::Process;

pub struct BotProcess {
    //
}

impl BotProcess {
    pub fn new() -> &'static Self {
        static INSTANCE: OnceLock<BotProcess> = OnceLock::new();
        INSTANCE.get_or_init(|| BotProcess {})
    }
}

#[async_trait]
impl Process for BotProcess {
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
            _ = tokio::time::sleep(time::Duration::from_secs(30)) => {
                    info!("process delayed");
            }
            }
        }
    }
}
