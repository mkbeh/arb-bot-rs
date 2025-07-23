use std::sync::Arc;

use anyhow::{anyhow, bail};
use app::{
    config::{Config, Exchange},
    cron::{arbitrage_job, order_sender_job},
    libs::{
        binance_api,
        binance_api::Binance,
        http_server::{Server, server::ServerProcess},
    },
    services::{
        BinanceExchangeConfig, BinanceExchangeService, BinanceSender, BinanceSenderConfig,
        ExchangeService, OrderSenderService, binance::REQUEST_WEIGHT,
    },
};

#[derive(Default)]
pub struct Entrypoint;

impl Entrypoint {
    pub async fn run(&self) -> anyhow::Result<()> {
        let config = Config::parse().map_err(|e| anyhow!("Failed to parse config file: {e}"))?;

        let exchange_service: Arc<dyn ExchangeService> =
            match config.settings.exchange_name.parse()? {
                Exchange::Binance => self.build_binance_exchange_service(config.clone()).await?,
            };

        let order_sender_service: Arc<dyn OrderSenderService> =
            match config.settings.exchange_name.parse()? {
                Exchange::Binance => self.build_binance_sender_service(config.clone()).await?,
            };

        let arbitrage_config =
            arbitrage_job::Config::new(config.settings.timeout, config.settings.error_timeout);
        let arbitrage_ps = arbitrage_job::Process::new(arbitrage_config, exchange_service);

        let order_sender_config = order_sender_job::Config::new(config.settings.error_timeout);
        let order_sender_ps =
            order_sender_job::Process::new(order_sender_config, order_sender_service);

        let processes: Vec<&dyn ServerProcess> = vec![arbitrage_ps, order_sender_ps];

        Server::new()
            .with_processes(&processes)
            .run()
            .await
            .map_err(|e| anyhow!("handling server error: {}", e))?;

        Ok(())
    }

    async fn build_binance_exchange_service(
        &self,
        config: Config,
    ) -> anyhow::Result<Arc<BinanceExchangeService>> {
        let api_config = binance_api::Config {
            api_url: config.binance.api_url,
            api_token: config.binance.api_token,
            api_secret_key: config.binance.api_secret_key,
            http_config: binance_api::HttpConfig::default(),
        };

        let general_api = match Binance::new(api_config.clone()) {
            Ok(v) => v,
            Err(e) => bail!("Failed init binance client: {e}"),
        };

        let market_api = match Binance::new(api_config.clone()) {
            Ok(v) => v,
            Err(e) => bail!("Failed init binance client: {e}"),
        };

        {
            let mut request_weight = REQUEST_WEIGHT.lock().await;
            request_weight.set_weight_limit(config.binance.api_weight_limit);
        }

        let service_config = BinanceExchangeConfig {
            general_api,
            market_api,
            base_assets: config.binance.assets,
            market_depth_limit: config.binance.market_depth_limit,
            default_min_profit_limit: config.settings.min_profit_limit,
            default_max_volume_limit: config.settings.max_volume_limit,
        };
        let service = Arc::new(BinanceExchangeService::new(service_config));

        Ok(service)
    }

    async fn build_binance_sender_service(
        &self,
        config: Config,
    ) -> anyhow::Result<Arc<BinanceSender>> {
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

        let trade_api = match Binance::new(api_config.clone()) {
            Ok(v) => v,
            Err(e) => bail!("Failed init binance client: {e}"),
        };

        let service_config = BinanceSenderConfig {
            account_api,
            trade_api,
            send_orders: config.settings.send_orders,
        };
        let service = Arc::new(BinanceSender::new(service_config));

        Ok(service)
    }
}
