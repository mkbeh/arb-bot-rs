use anyhow::bail;
use rust_decimal::Decimal;
use serde::Deserialize;

/// General application settings.
#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub api_url: String,
    pub api_token: String,
    pub api_secret_key: String,
    pub ws_url: String,
    pub ws_streams_url: String,
    pub ws_max_connections: usize,
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
    pub skip_assets: Vec<String>,
    pub assets: Vec<Asset>,
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
    pub fn validate(
        &mut self,
        min_profit_qty: Decimal,
        max_order_qty: Decimal,
        min_ticker_qty_24h: Decimal,
    ) -> anyhow::Result<()> {
        match &self.symbol {
            Some(symbol) => {
                if !symbol.contains("USDT") {
                    bail!("Symbol must contain 'USDT': {symbol}");
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
