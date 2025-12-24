use anyhow::{Context, bail};
use rust_decimal::Decimal;
use serde::Deserialize;
use strum_macros::EnumString;

use crate::libs::toml;

/// Default configuration file name.
const CONFIG_FILE: &str = "config.toml";

/// Enumeration of supported exchanges.
#[derive(Debug, PartialEq, EnumString)]
pub enum ExchangeType {
    #[strum(serialize = "binance")]
    Binance,
    #[strum(serialize = "kucoin")]
    Kucoin,
    #[strum(serialize = "solana-dex")]
    SolanaDex,
}

/// Main application configuration structure.
/// Contains general settings, as well as exchange-specific configurations.
#[derive(Clone, Deserialize)]
pub struct Config {
    #[serde(rename = "settings")]
    pub settings: Settings,
    #[serde(rename = "binance-settings")]
    pub binance: BinanceSettings,
    #[serde(rename = "kucoin-settings")]
    pub kucoin: KucoinSettings,
    #[serde(rename = "solana-dex-settings")]
    pub solana_dex: SolanaDexSettings,
}

impl Config {
    /// Parses the configuration from the TOML file and performs validation.
    pub fn parse() -> anyhow::Result<Self> {
        let config = toml::parse_file::<Config>(CONFIG_FILE)
            .with_context(|| format!("Failed to parse config file: {}", CONFIG_FILE))?;

        config.validate_settings()
    }

    /// Validates settings: checks the presence of assets and their parameters.
    fn validate_settings(self) -> anyhow::Result<Self> {
        let mut config = self;

        if config.settings.assets.is_empty() {
            bail!("At least one asset must be specified in config");
        }

        let min_profit_qty = config.settings.min_profit_qty;
        let max_order_qty = config.settings.max_order_qty;
        let min_ticker_qty_24h = config.settings.min_ticker_qty_24h;

        for asset in &mut config.settings.assets {
            asset.validate(min_profit_qty, max_order_qty, min_ticker_qty_24h)?;
        }

        Ok(config)
    }
}

/// General application settings.
#[derive(Clone, Deserialize)]
pub struct Settings {
    pub server_addr: String,
    pub metrics_addr: String,
    pub exchange_name: String,
    #[serde(with = "rust_decimal::serde::float")]
    pub fee_percent: Decimal,
    pub api_weight_limit: usize,
    pub error_timeout: u64,
    pub send_orders: bool,
    #[serde(with = "rust_decimal::serde::float")]
    pub min_profit_qty: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    pub max_order_qty: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    pub min_ticker_qty_24h: Decimal,
    pub market_depth_limit: usize,
    pub assets: Vec<Asset>,
    pub skip_assets: Vec<String>,
}

/// Settings for Binance API.
#[derive(Clone, Deserialize)]
pub struct BinanceSettings {
    pub api_url: String,
    pub api_token: String,
    pub api_secret_key: String,
    pub ws_url: String,
    pub ws_streams_url: String,
    pub ws_max_connections: usize,
}

/// Settings for KuCoin API.
#[derive(Clone, Deserialize)]
pub struct KucoinSettings {
    pub api_url: String,
    pub api_token: String,
    pub api_secret_key: String,
    pub api_passphrase: String,
    pub ws_private_url: String,
}

#[derive(Clone, Deserialize)]
pub struct SolanaDexSettings {
    pub rpc_endpoint: String,
    pub grpc_endpoint: String,
    pub x_token: Option<String>,
    pub exchanges: Vec<Dex>,
}

impl SolanaDexSettings {
    pub fn get_dex_programs(&self) -> Vec<String> {
        self.exchanges
            .iter()
            .map(|d| d.program_id.clone())
            .collect()
    }
}

/// Asset structure for arbitrage.
/// Describes the base asset and trading limit parameters.
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

impl Asset {
    /// Validates the asset parameters and sets default values if symbol is missing.
    fn validate(
        &mut self,
        min_profit_qty: Decimal,
        max_order_qty: Decimal,
        min_ticker_qty_24h: Decimal,
    ) -> anyhow::Result<()> {
        match &self.symbol {
            Some(symbol) => {
                if !symbol.contains("USDT") {
                    bail!("Symbol must contain 'USDT': {}", symbol);
                }
            }
            None => {
                // Set default limits only if all fields
                // are zero (signal of no overrides).
                if self.min_profit_qty.is_zero()
                    && self.max_order_qty.is_zero()
                    && self.min_ticker_qty_24h.is_zero()
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

#[derive(Deserialize, Clone, Debug)]
pub struct Dex {
    pub program_id: String,
}
