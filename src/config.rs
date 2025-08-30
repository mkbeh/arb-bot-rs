use std::str::FromStr;

use anyhow::{anyhow, bail};
use rust_decimal::{Decimal, prelude::Zero};
use serde::Deserialize;
use strum_macros::EnumString;

use crate::libs::toml;

const CONFIG_FILE: &str = "config.toml";
const MAX_MARKET_DEPTH_LIMIT: usize = 20;

#[derive(Debug, PartialEq, EnumString)]
pub enum Exchange {
    #[strum(serialize = "binance")]
    Binance,
}

#[derive(Clone, Deserialize)]
pub struct Config {
    #[serde(rename = "settings")]
    pub settings: Settings,
    #[serde(rename = "binance-settings")]
    pub binance: BinanceSettings,
}

#[derive(Clone, Deserialize)]
pub struct Settings {
    pub exchange_name: String,
    #[serde(with = "rust_decimal::serde::float")]
    pub fee_percent: Decimal,
    pub error_timeout: u64,
    pub order_lifetime: u64,
    pub send_orders: bool,
    #[serde(with = "rust_decimal::serde::float")]
    pub min_profit_qty: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    pub max_order_qty: Decimal,
}

#[derive(Clone, Deserialize)]
pub struct BinanceSettings {
    pub api_url: String,
    pub api_token: String,
    pub api_secret_key: String,
    pub api_weight_limit: usize,
    pub ws_url: String,
    pub ws_streams_url: String,
    pub ws_max_connections: usize,
    pub market_depth_limit: usize,
    pub assets: Vec<Asset>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Asset {
    pub asset: String,
    pub symbol: Option<String>,
    #[serde(with = "rust_decimal::serde::float")]
    pub min_profit_qty: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    pub max_order_qty: Decimal,
}

impl Asset {
    fn check(&mut self, min_profit_qty: Decimal, max_order_qty: Decimal) -> anyhow::Result<()> {
        match self.symbol.as_ref() {
            Some(symbol) => {
                if !symbol.contains("USDT") {
                    bail!("Symbol must contains USDT asset: {}", symbol);
                }
            }
            None => {
                // Set default limits if symbol not present in config.
                if self.max_order_qty == Decimal::zero() && self.min_profit_qty == Decimal::zero() {
                    self.min_profit_qty = min_profit_qty;
                    self.max_order_qty = max_order_qty;
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

        if self.settings.max_order_qty <= Decimal::zero() {
            bail!("max_order_qty must be greater than 0");
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

        if self.binance.api_weight_limit == 0 {
            bail!("weight_limit must be greater than 0");
        }

        for asset in &mut self.binance.assets {
            asset.check(self.settings.min_profit_qty, self.settings.max_order_qty)?;
        }

        Ok(())
    }
}
