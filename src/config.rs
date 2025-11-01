use anyhow::{anyhow, bail};
use rust_decimal::{Decimal, prelude::Zero};
use serde::Deserialize;
use strum_macros::EnumString;

use crate::libs::toml;

const CONFIG_FILE: &str = "config.toml";

#[derive(Debug, PartialEq, EnumString)]
pub enum Exchange {
    #[strum(serialize = "binance")]
    Binance,
    #[strum(serialize = "kucoin")]
    Kucoin,
}

#[derive(Clone, Deserialize)]
pub struct Config {
    #[serde(rename = "settings")]
    pub settings: Settings,
    #[serde(rename = "binance-settings")]
    pub binance: BinanceSettings,
    #[serde(rename = "kucoin-settings")]
    pub kucoin: KucoinSettings,
}

#[derive(Clone, Deserialize)]
pub struct Settings {
    pub server_addr: String,
    pub metrics_addr: String,
    pub exchange_name: String,
    #[serde(with = "rust_decimal::serde::float")]
    pub fee_percent: Decimal,
    pub api_weight_limit: usize,
    pub error_timeout: u64,
    pub order_lifetime: u64,
    pub send_orders: bool,
    #[serde(with = "rust_decimal::serde::float")]
    pub min_profit_qty: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    pub max_order_qty: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    pub min_ticker_qty_24h: Decimal,
    pub assets: Vec<Asset>,
}

#[derive(Clone, Deserialize)]
pub struct BinanceSettings {
    pub api_url: String,
    pub api_token: String,
    pub api_secret_key: String,
    pub ws_url: String,
    pub ws_streams_url: String,
    pub ws_max_connections: usize,
    pub market_depth_limit: usize,
}

#[derive(Clone, Deserialize)]
pub struct KucoinSettings {
    pub api_url: String,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Asset {
    pub asset: String,
    pub symbol: Option<String>,
    #[serde(with = "rust_decimal::serde::float")]
    pub min_profit_qty: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    pub max_order_qty: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    pub min_ticker_qty_24h: Decimal,
}

impl Config {
    pub fn parse() -> anyhow::Result<Self> {
        let mut config: Config = toml::parse_file(CONFIG_FILE).map_err(|e| anyhow!("{}", e))?;

        if let Err(e) = config.validate_settings() {
            bail!("Config validation error: {}", e)
        }

        Ok(config)
    }

    fn validate_settings(&mut self) -> anyhow::Result<()> {
        if self.settings.assets.is_empty() {
            bail!("At least one asset must be specified");
        }

        for asset in &mut self.settings.assets {
            asset.check(
                self.settings.min_profit_qty,
                self.settings.max_order_qty,
                self.settings.min_ticker_qty_24h,
            )?;
        }

        Ok(())
    }
}

impl Asset {
    fn check(
        &mut self,
        min_profit_qty: Decimal,
        max_order_qty: Decimal,
        min_ticker_qty_24h: Decimal,
    ) -> anyhow::Result<()> {
        match self.symbol.as_ref() {
            Some(symbol) => {
                if !symbol.contains("USDT") {
                    bail!("Symbol must contains USDT asset: {}", symbol);
                }
            }
            None => {
                // Set default limits if symbol not present in config.
                if self.max_order_qty == Decimal::zero()
                    && self.min_profit_qty == Decimal::zero()
                    && self.min_ticker_qty_24h == Decimal::zero()
                {
                    self.min_profit_qty = min_profit_qty;
                    self.max_order_qty = max_order_qty;
                    self.min_ticker_qty_24h = min_ticker_qty_24h;
                }
            }
        }

        Ok(())
    }
}
