use std::str::FromStr;

use anyhow::{anyhow, bail};
use serde::Deserialize;
use strum_macros::EnumString;

use crate::libs::toml;

const CONFIG_FILE: &str = "config.toml";

#[derive(Debug, PartialEq, EnumString)]
pub enum Exchange {
    #[strum(serialize = "binance")]
    Binance,
}

#[derive(Deserialize)]
pub struct Config {
    pub settings: Settings,
}

#[derive(Deserialize)]
pub struct Settings {
    pub exchange_name: String,
    pub exchange_api_url: String,
    pub exchange_api_token: String,
    pub exchange_api_secret_key: String,
    pub delay: u64,
    pub base_assets: Vec<String>,
}

impl Config {
    pub fn parse() -> anyhow::Result<Self> {
        let config: Config = toml::parse_file(CONFIG_FILE).map_err(|e| anyhow!("{}", e))?;
        match config.validate() {
            Ok(_) => Ok(config),
            Err(e) => bail!("{}", e),
        }
    }

    fn validate(&self) -> anyhow::Result<()> {
        Exchange::from_str(&self.settings.exchange_name).map_err(|_| {
            anyhow!(
                "exchange_name '{}' does not exist:",
                self.settings.exchange_name
            )
        })?;

        if self.settings.base_assets.is_empty() {
            bail!("base_assets is empty");
        }

        Ok(())
    }
}
