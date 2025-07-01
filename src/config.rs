use std::str::FromStr;

use anyhow::{anyhow, bail};
use serde::Deserialize;
use strum_macros::EnumString;

use crate::libs::toml;

const CONFIG_FILE: &str = "config.toml";
const MAX_DELAY: u64 = 5000;
const MAX_MARKET_DEPTH_LIMIT: usize = 20;

#[derive(Debug, PartialEq, EnumString)]
pub enum Exchange {
    #[strum(serialize = "binance")]
    Binance,
}

#[derive(Deserialize)]
pub struct Config {
    pub base: BaseSettings,
    pub binance: BinanceSettings,
}

#[derive(Deserialize)]
pub struct BaseSettings {
    pub exchange_name: String,
    pub delay: u64,
}

#[derive(Deserialize)]
pub struct BinanceSettings {
    pub exchange_api_url: String,
    pub exchange_api_token: String,
    pub exchange_api_secret_key: String,
    pub base_assets: Vec<String>,
    pub market_depth_limit: usize,
}

impl Config {
    pub fn parse() -> anyhow::Result<Self> {
        let config: Config = toml::parse_file(CONFIG_FILE).map_err(|e| anyhow!("{}", e))?;

        if let Err(e) = config.validate_base() {
            bail!("Config validation error: {}", e)
        }

        if let Err(e) = config.validate_binance() {
            bail!("Config validation error: {}", e)
        }

        Ok(config)
    }

    fn validate_base(&self) -> anyhow::Result<()> {
        Exchange::from_str(&self.base.exchange_name).map_err(|_| {
            anyhow!(
                "exchange_name '{}' does not exist:",
                self.base.exchange_name
            )
        })?;

        if self.base.delay > MAX_DELAY {
            bail!("delay is greater than: {}", MAX_DELAY);
        }

        Ok(())
    }

    fn validate_binance(&self) -> anyhow::Result<()> {
        if self.binance.base_assets.is_empty() {
            bail!("base_assets is empty");
        }

        if self.binance.market_depth_limit > MAX_MARKET_DEPTH_LIMIT {
            bail!(
                "market_depth_limit is greater than {}",
                MAX_MARKET_DEPTH_LIMIT
            );
        }

        Ok(())
    }
}
