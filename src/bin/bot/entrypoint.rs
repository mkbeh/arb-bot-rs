use std::sync::Arc;

use anyhow::{anyhow, bail};
use app::{
    config::{Config, Exchange, Settings},
    cron::arbitrage_job,
    libs::{binance_api, binance_api::Binance, http_server::Server},
    services::{BinanceService, ExchangeService},
};

#[derive(Default)]
pub struct Entrypoint;

impl Entrypoint {
    pub async fn run(&self) -> anyhow::Result<()> {
        let config = Config::parse().map_err(|e| anyhow!("Failed to parse config file: {e}"))?;

        let job_cfg = arbitrage_job::Config {
            delay: config.settings.delay,
        };

        let exchange_service: Arc<dyn ExchangeService> =
            match config.settings.exchange_name.parse()? {
                Exchange::Binance => self.build_binance_service(config.settings)?,
            };

        let job_ps = arbitrage_job::Process::new(job_cfg, exchange_service);

        Server::new()
            .with_processes(&vec![job_ps])
            .run()
            .await
            .map_err(|e| anyhow!("handling server error: {}", e))?;

        Ok(())
    }

    fn build_binance_service(&self, settings: Settings) -> anyhow::Result<Arc<BinanceService>> {
        let cfg = binance_api::Config {
            host: settings.exchange_api_url,
            api_key: settings.exchange_api_token,
            secret_key: settings.exchange_api_secret_key,
            http_config: binance_api::HttpConfig::default(),
        };

        let client = match Binance::new(cfg) {
            Ok(v) => v,
            Err(e) => bail!("Failed init binance client: {e}"),
        };

        Ok(Arc::new(BinanceService::new(client)))
    }
}
