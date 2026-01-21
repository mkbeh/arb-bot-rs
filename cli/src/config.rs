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

#[allow(dead_code)]
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
    pub fn load(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        toml::parse_file::<Self>(path)?.validate()
    }

    #[allow(unused_mut)]
    pub fn validate(mut self) -> anyhow::Result<Self> {
        use engine::Validatable;

        let configs: Vec<Option<&mut dyn Validatable>> = vec![
            #[cfg(feature = "binance")]
            self.binance.as_mut().map(|c| c as &mut dyn Validatable),
            #[cfg(feature = "kucoin")]
            self.kucoin.as_mut().map(|c| c as &mut dyn Validatable),
            #[cfg(feature = "solana")]
            self.solana.as_mut().map(|c| c as &mut dyn Validatable),
        ];

        for cfg in configs.into_iter().flatten() {
            cfg.validate()?;
        }

        Ok(self)
    }
}
