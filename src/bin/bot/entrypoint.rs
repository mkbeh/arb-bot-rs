use std::sync::LazyLock;

use anyhow::anyhow;
use app::{
    cron::bot,
    libs::{http_server::Server, toml},
};
use serde_derive::Deserialize;

const CONFIG_FILE: &str = "config.toml";
static EXCHANGES: LazyLock<Vec<&str>> = LazyLock::new(|| vec!["binance"]);

#[derive(Deserialize)]
pub struct Config {
    pub settings: Settings,
}

#[derive(Deserialize)]
pub struct Settings {
    pub exchange_name: String,
    pub exchange_api_url: String,
    pub exchange_api_token: String,
    pub delay: u64,
}

impl Config {
    fn parse(filename: &str) -> anyhow::Result<Self> {
        let config: Config = toml::parse_file(filename).map_err(|e| anyhow!("{}", e))?;
        match config.validate() {
            Ok(_) => Ok(config),
            Err(e) => Err(anyhow!("{}", e)),
        }
    }

    fn validate(&self) -> anyhow::Result<()> {
        if !EXCHANGES.contains(&self.settings.exchange_name.as_str()) {
            return Err(anyhow!(
                "Exchange {} not available",
                self.settings.exchange_name
            ));
        }

        Ok(())
    }
}

pub struct Entrypoint;

impl Entrypoint {
    pub fn new() -> Self {
        Self {}
    }

    pub async fn run(&self) -> anyhow::Result<()> {
        let config =
            Config::parse(CONFIG_FILE).map_err(|e| anyhow!("Failed to parse config file: {e}"))?;

        let bot_ps = bot::Process::new(bot::Config {
            delay: config.settings.delay,
        });

        Server::new()
            .with_processes(&vec![bot_ps])
            .run()
            .await
            .map_err(|e| anyhow!("handling server error: {}", e))?;

        Ok(())
    }
}
