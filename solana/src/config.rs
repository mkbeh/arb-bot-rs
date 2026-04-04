use std::time::Duration;

use ahash::{AHashSet, HashSet};
use anyhow::bail;
use engine::Validatable;
use serde::Deserialize;
use serde_with::{DisplayFromStr, DurationMicroSeconds, serde_as};
use solana_sdk::pubkey::Pubkey;

use crate::libs::solana_client::{GrpcStreamConfig, RpcConfig, WebsocketStreamConfig};

#[derive(Deserialize, Clone, Debug)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum TransportConfig {
    Websocket { url: String },
    Grpc { url: String, x_token: String },
}

#[serde_as]
#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub rpc_endpoint: String,
    pub transport: TransportConfig,

    pub stream_batch_size: usize,
    #[serde_as(as = "DurationMicroSeconds<u64>")]
    pub stream_wait_timeout_us: Duration,
    pub liquidity_depth: i64,

    pub exchanges: HashSet<ProtocolConfig>,
    pub base_mints: HashSet<MintConfig>,
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
    pub fn get_dex_addrs(&self) -> Vec<String> {
        self.exchanges
            .iter()
            .map(|d| d.program_id.clone())
            .collect()
    }

    #[must_use]
    pub fn get_mints_addrs(&self) -> AHashSet<Pubkey> {
        self.base_mints.iter().map(|c| c.mint_addr).collect()
    }

    #[must_use]
    pub fn get_reserves_addrs(&self) -> AHashSet<Pubkey> {
        self.base_mints
            .iter()
            .filter_map(|c| c.reserve_addr)
            .collect()
    }
}

#[derive(Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
pub struct ProtocolConfig {
    pub program_id: String,
}

#[serde_as]
#[derive(Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
pub struct MintConfig {
    #[serde_as(as = "DisplayFromStr")]
    pub mint_addr: Pubkey,
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub reserve_addr: Option<Pubkey>,
}

//  --- CONVERSION TRAITS ---

impl TryFrom<&Config> for RpcConfig {
    type Error = anyhow::Error;

    fn try_from(cfg: &Config) -> Result<Self, Self::Error> {
        Ok(Self {
            url: cfg.rpc_endpoint.clone(),
        })
    }
}

impl TryFrom<&Config> for WebsocketStreamConfig {
    type Error = anyhow::Error;

    fn try_from(cfg: &Config) -> Result<Self, Self::Error> {
        let endpoint = match &cfg.transport {
            TransportConfig::Websocket { url } => url.clone(),
            TransportConfig::Grpc { .. } => bail!("Transport is not set to 'websocket'"),
        };

        Ok(Self {
            endpoint,
            ping_interval: Duration::from_secs(15),
            batch_size: cfg.stream_batch_size,
            batch_fill_timeout: cfg.stream_wait_timeout_us,
            ..Default::default()
        })
    }
}

impl TryFrom<&Config> for GrpcStreamConfig {
    type Error = anyhow::Error;

    fn try_from(cfg: &Config) -> Result<Self, Self::Error> {
        let (endpoint, x_token) = match &cfg.transport {
            TransportConfig::Grpc { url, x_token } => (url.clone(), Some(x_token.clone())),
            TransportConfig::Websocket { .. } => bail!("Transport is not set to 'grpc'"),
        };

        Ok(Self {
            endpoint,
            x_token,
            batch_size: cfg.stream_batch_size,
            batch_fill_timeout: cfg.stream_wait_timeout_us,
            ..Default::default()
        })
    }
}
