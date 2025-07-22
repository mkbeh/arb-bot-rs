use std::{
    sync::{Arc, OnceLock},
    time::Duration,
};

use async_trait::async_trait;
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

use crate::{
    libs::http_server::ServerProcess,
    services::{OrderSenderService, service::ORDERS_CHANNEL},
};

pub struct Config {
    pub error_timeout_secs: u64,
}

impl Config {
    pub fn new(error_timeout_secs: u64) -> Self {
        Self { error_timeout_secs }
    }
}

pub struct Process {
    error_timeout_secs: Duration,
    service: Arc<dyn OrderSenderService>,
}

impl Process {
    pub fn new(cfg: Config, service: Arc<dyn OrderSenderService>) -> &'static Self {
        static INSTANCE: OnceLock<Process> = OnceLock::new();
        INSTANCE.get_or_init(|| Process {
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
        let mut orders_rx = ORDERS_CHANNEL.rx.lock().await;
        loop {
            tokio::select! {
                _ = token.cancelled() => {
                    // todo: need close orders chan
                    return Ok(());
                }
                Some(msg) = orders_rx.recv() => {
                    match self.service.send_orders(msg).await {
                        Ok(_) => info!("orders send process complete successfully"),
                        Err(e) => {
                            error!(error = ?e, "error during orders send process");
                            sleep(self.error_timeout_secs).await;
                        },
                    };
                }
            }
        }
    }
}
