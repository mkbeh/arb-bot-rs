use std::str::FromStr;

use anyhow::{anyhow, bail};
use rust_decimal::{Decimal, prelude::Zero};
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
    #[serde(rename = "settings")]
    pub settings: Settings,
    #[serde(rename = "binance-settings")]
    pub binance: BinanceSettings,
}

#[derive(Deserialize)]
pub struct Settings {
    pub exchange_name: String,
    pub timeout: u64,
    pub error_timeout: u64,
    #[serde(with = "rust_decimal::serde::float")]
    pub min_profit_limit: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    pub max_volume_limit: Decimal,
}

#[derive(Deserialize)]
pub struct BinanceSettings {
    pub api_url: String,
    pub api_token: String,
    pub api_secret_key: String,
    pub market_depth_limit: usize,
    pub assets: Vec<Asset>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Asset {
    pub asset: String,
    pub symbol: Option<String>,
    #[serde(with = "rust_decimal::serde::float")]
    pub min_profit_limit: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    pub max_volume_limit: Decimal,
}

impl Asset {
    fn check(
        &mut self,
        min_profit_limit: Decimal,
        max_volume_limit: Decimal,
    ) -> anyhow::Result<()> {
        match self.symbol.as_ref() {
            Some(symbol) => {
                if !symbol.contains("USDT") {
                    bail!("Symbol must contains USDT asset: {}", symbol);
                }
            }
            None => {
                // Set default limits if symbol not present in config.
                if self.max_volume_limit == Decimal::zero()
                    && self.min_profit_limit == Decimal::zero()
                {
                    self.min_profit_limit = min_profit_limit;
                    self.max_volume_limit = max_volume_limit;
                }
            }
        }

        Ok(())
    }
}

impl Config {
    pub fn parse() -> anyhow::Result<Self> {
        let mut config: Config = toml::parse_file(CONFIG_FILE).map_err(|e| anyhow!("{}", e))?;

        if let Err(e) = config.validate_settings() {
            bail!("Config validation error: {}", e)
        }

        if let Err(e) = config.validate_binance_settings() {
            bail!("Config validation error: {}", e)
        }

        Ok(config)
    }

    fn validate_settings(&self) -> anyhow::Result<()> {
        Exchange::from_str(&self.settings.exchange_name).map_err(|_| {
            anyhow!(
                "exchange_name '{}' does not exist:",
                self.settings.exchange_name
            )
        })?;

        if self.settings.timeout > MAX_DELAY {
            bail!("delay is greater than: {}", MAX_DELAY);
        }

        if self.settings.min_profit_limit <= Decimal::zero() {
            bail!("min_profit_limit is greater than 0");
        }

        if self.settings.max_volume_limit <= Decimal::zero() {
            bail!("max_volume_limit is greater than 0");
        }

        Ok(())
    }

    fn validate_binance_settings(&mut self) -> anyhow::Result<()> {
        if self.binance.assets.is_empty() {
            bail!("At least one asset must be specified");
        }

        if self.binance.market_depth_limit > MAX_MARKET_DEPTH_LIMIT {
            bail!(
                "market_depth_limit is greater than {}",
                MAX_MARKET_DEPTH_LIMIT
            );
        }

        for asset in &mut self.binance.assets {
            asset.check(
                self.settings.min_profit_limit,
                self.settings.max_volume_limit,
            )?;
        }

        Ok(())
    }
}
