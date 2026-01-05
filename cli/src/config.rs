use std::path::Path;

use serde::Deserialize;
use tools::toml;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    #[cfg(feature = "binance")]
    pub binance: Option<binance::Config>,

    #[cfg(feature = "kucoin")]
    pub kucoin: Option<kucoin::Config>,

    #[cfg(feature = "solana")]
    pub solana: Option<solana::Config>,

    #[allow(dead_code)]
    pub general: GeneralConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct GeneralConfig {
    pub server_addr: String,
    pub metrics_addr: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            #[cfg(feature = "binance")]
            binance: None,

            #[cfg(feature = "kucoin")]
            kucoin: None,

            #[cfg(feature = "solana")]
            solana: None,

            general: GeneralConfig {
                server_addr: "127.0.0.1:9000".to_owned(),
                metrics_addr: "127.0.0.1:9007".to_owned(),
            },
        }
    }
}

impl Config {
    pub fn parse(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let mut config = toml::parse_file::<Self>(path)?;
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&mut self) -> anyhow::Result<()> {
        #[cfg(feature = "binance")]
        if let Some(ref mut cfg) = self.binance {
            for asset in &mut cfg.assets.iter_mut() {
                asset.validate(
                    cfg.min_profit_qty,
                    cfg.max_order_qty,
                    cfg.min_ticker_qty_24h,
                )?;
            }
        }

        #[cfg(feature = "kucoin")]
        if let Some(ref mut cfg) = self.kucoin {
            for asset in &mut cfg.assets.iter_mut() {
                asset.validate(
                    cfg.min_profit_qty,
                    cfg.max_order_qty,
                    cfg.min_ticker_qty_24h,
                )?;
            }
        }

        #[cfg(feature = "solana")]
        if let Some(ref mut cfg) = self.solana {
            cfg.validate()?
        }

        Ok(())
    }
}
