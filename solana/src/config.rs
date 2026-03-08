use std::time::Duration;

use anyhow::{anyhow, bail};
use engine::Validatable;
use serde::Deserialize;
use serde_with::{DurationMicroSeconds, serde_as};

use crate::libs::solana_client::{GrpcConfig, RpcConfig, StreamConfig};

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Transport {
    Websocket,
    Grpc,
}

#[serde_as]
#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub transport: Transport,
    pub rpc_endpoint: String,
    pub grpc_endpoint: Option<String>,
    pub x_token: Option<String>,
    pub ws_endpoint: Option<String>,
    pub stream_batch_size: usize,
    #[serde_as(as = "DurationMicroSeconds<u64>")]
    pub stream_wait_timeout_us: Duration,
    pub liquidity_depth: i64,
    pub exchanges: Vec<Dex>,
}

impl Validatable for Config {
    fn validate(&mut self) -> anyhow::Result<()> {
        if self.rpc_endpoint.is_empty() {
            bail!("RPC endpoint cannot be empty");
        }
        Ok(())
    }
}

impl Config {
    #[must_use]
    pub fn get_dex_programs(&self) -> Vec<String> {
        self.exchanges
            .iter()
            .map(|d| d.program_id.clone())
            .collect()
    }
}

#[derive(Deserialize, Clone, Debug)]
pub struct Dex {
    pub program_id: String,
}

// ==========================================
// CONVERSION TRAITS (Data Mapping)
// ==========================================

impl TryFrom<&Config> for RpcConfig {
    type Error = anyhow::Error;

    fn try_from(cfg: &Config) -> Result<Self, Self::Error> {
        Ok(Self {
            url: cfg.rpc_endpoint.clone(),
        })
    }
}

impl TryFrom<&Config> for StreamConfig {
    type Error = anyhow::Error;

    fn try_from(cfg: &Config) -> Result<Self, Self::Error> {
        Ok(Self {
            endpoint: cfg
                .ws_endpoint
                .clone()
                .ok_or_else(|| anyhow!("ws_endpoint missing"))?,
            ping_interval: Duration::from_secs(15),
            batch_size: cfg.stream_batch_size,
            batch_fill_timeout: cfg.stream_wait_timeout_us,
            program_ids: cfg.get_dex_programs(),
            targets: vec![],
        })
    }
}

impl TryFrom<&Config> for GrpcConfig {
    type Error = anyhow::Error;

    fn try_from(cfg: &Config) -> Result<Self, Self::Error> {
        Ok(Self {
            endpoint: cfg
                .grpc_endpoint
                .clone()
                .ok_or_else(|| anyhow!("grpc_endpoint missing"))?,
            x_token: cfg.x_token.clone(),
            batch_size: cfg.stream_batch_size,
            batch_fill_timeout: cfg.stream_wait_timeout_us,
            program_ids: cfg.get_dex_programs(),
            targets: vec![],
            ..Default::default()
        })
    }
}
