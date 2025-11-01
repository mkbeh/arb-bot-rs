use std::sync::Arc;

use anyhow::bail;
use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

use crate::{
    config::{Asset, Config},
    libs::{kucoin_api, kucoin_api::Kucoin},
    services::{ExchangeService, kucoin::exchange::chain::ChainBuilder},
};

pub struct KucoinExchangeConfig {
    pub base_assets: Vec<Asset>,
    pub api_url: String,
}

pub struct KucoinExchangeService {
    base_assets: Vec<Asset>,
    chain_builder: Arc<ChainBuilder>,
}

impl From<&Config> for KucoinExchangeConfig {
    fn from(config: &Config) -> Self {
        Self {
            base_assets: config.settings.assets.clone(),
            api_url: config.kucoin.api_url.clone(),
        }
    }
}

impl KucoinExchangeService {
    pub fn from_config(config: KucoinExchangeConfig) -> anyhow::Result<Self> {
        let api_config = kucoin_api::ClientConfig {
            host: config.api_url,
            http_config: kucoin_api::HttpConfig::default(),
        };

        let market_api = match Kucoin::new(api_config) {
            Ok(client) => client,
            Err(e) => bail!("Failed init kucoin client: {e}"),
        };

        let chain_builder = ChainBuilder::new(market_api);

        Ok(Self {
            base_assets: config.base_assets,
            chain_builder: Arc::new(chain_builder),
        })
    }
}

#[async_trait]
impl ExchangeService for KucoinExchangeService {
    async fn start_arbitrage(&self, token: CancellationToken) -> anyhow::Result<()> {
        let chains = match self
            .chain_builder
            .clone()
            .build_symbols_chains(self.base_assets.clone())
            .await
        {
            Ok(chains) => chains,
            Err(e) => bail!("failed to build symbols chains: {}", e),
        };

        Ok(())
    }
}
