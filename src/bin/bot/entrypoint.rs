use std::sync::Arc;

use anyhow::{anyhow, bail};
use app::{
    config::{Config, Exchange},
    cron::arbitrage_job,
    libs::{binance_api, binance_api::Binance, http_server::Server},
    services::{BinanceConfig, BinanceService, ExchangeService},
};

#[derive(Default)]
pub struct Entrypoint;

impl Entrypoint {
    pub async fn run(&self) -> anyhow::Result<()> {
        let config = Config::parse().map_err(|e| anyhow!("Failed to parse config file: {e}"))?;

        let job_cfg = arbitrage_job::Config {
            timeout: config.settings.timeout,
            error_timeout: config.settings.error_timeout,
        };

        let exchange_service: Arc<dyn ExchangeService> =
            match config.settings.exchange_name.parse()? {
                Exchange::Binance => self.build_binance_service(config)?,
            };

        let job_ps = arbitrage_job::Process::new(job_cfg, exchange_service);

        Server::new()
            .with_processes(&vec![job_ps])
            .run()
            .await
            .map_err(|e| anyhow!("handling server error: {}", e))?;

        Ok(())
    }

    fn build_binance_service(&self, config: Config) -> anyhow::Result<Arc<BinanceService>> {
        let api_config = binance_api::Config {
            api_url: config.binance.api_url,
            api_token: config.binance.api_token,
            api_secret_key: config.binance.api_secret_key,
            http_config: binance_api::HttpConfig::default(),
        };

        let account_api = match Binance::new(api_config.clone()) {
            Ok(v) => v,
            Err(e) => bail!("Failed init binance client: {e}"),
        };

        let general_api = match Binance::new(api_config.clone()) {
            Ok(v) => v,
            Err(e) => bail!("Failed init binance client: {e}"),
        };

        let market_api = match Binance::new(api_config.clone()) {
            Ok(v) => v,
            Err(e) => bail!("Failed init binance client: {e}"),
        };

        let trade_api = match Binance::new(api_config.clone()) {
            Ok(v) => v,
            Err(e) => bail!("Failed init binance client: {e}"),
        };

        let service_config = BinanceConfig {
            account_api,
            general_api,
            market_api,
            trade_api,
            base_assets: config.binance.assets,
            market_depth_limit: config.binance.market_depth_limit,
            default_min_profit_limit: config.settings.min_profit_limit,
            default_min_volume_limit: config.settings.min_volume_limit,
            default_max_volume_limit: config.settings.max_volume_limit,
        };

        Ok(Arc::new(BinanceService::new(service_config)))
    }
}
