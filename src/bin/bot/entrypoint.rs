use std::sync::Arc;

use anyhow::anyhow;
use app::{
    config::{Config, Exchange},
    cron::{arbitrage_job, order_sender_job},
    libs::http_server::{Server, server::ServerProcess},
    services::{
        BinanceExchangeConfig, BinanceExchangeService, BinanceSenderConfig, BinanceSenderService,
        ExchangeService, KucoinExchangeConfig, KucoinExchangeService, KucoinSenderConfig,
        KucoinSenderService, OrderSenderService,
    },
};

pub struct Entrypoint;

impl Entrypoint {
    pub async fn run(&self) -> anyhow::Result<()> {
        let config = Config::parse().map_err(|e| anyhow!("Failed to parse config file: {e}"))?;
        let settings = &config.settings;

        let exchange_service: Arc<dyn ExchangeService> =
            define_exchange_service(settings.exchange_name.clone(), config.clone()).await?;
        let sender_service: Arc<dyn OrderSenderService> =
            define_sender_service(settings.exchange_name.clone(), config.clone()).await?;

        let arbitrage_config = arbitrage_job::Config::new(settings.error_timeout);
        let arbitrage_ps = arbitrage_job::Process::new(arbitrage_config, exchange_service);

        let sender_config = order_sender_job::Config::new(settings.error_timeout);
        let sender_ps = order_sender_job::Process::new(sender_config, sender_service);

        let processes: Vec<&dyn ServerProcess> = vec![arbitrage_ps, sender_ps];

        Server::new(settings.server_addr.clone(), settings.metrics_addr.clone())
            .with_processes(&processes)
            .run()
            .await
            .map_err(|e| anyhow!("handling server error: {}", e))?;

        Ok(())
    }
}

async fn define_exchange_service(
    exchange_name: String,
    config: Config,
) -> anyhow::Result<Arc<dyn ExchangeService>> {
    match exchange_name.parse()? {
        Exchange::Binance => {
            let config = BinanceExchangeConfig::build(config.clone()).await?;
            Ok(Arc::new(BinanceExchangeService::from_config(config)))
        }
        Exchange::Kucoin => {
            let config = KucoinExchangeConfig::build(config.clone());
            Ok(Arc::new(KucoinExchangeService::from_config(config)))
        }
    }
}

async fn define_sender_service(
    exchange_name: String,
    config: Config,
) -> anyhow::Result<Arc<dyn OrderSenderService>> {
    match exchange_name.parse()? {
        Exchange::Binance => {
            let config = BinanceSenderConfig::build(config.clone());
            Ok(Arc::new(BinanceSenderService::from_config(config)))
        }
        Exchange::Kucoin => {
            let config = KucoinSenderConfig::build(config.clone());
            Ok(Arc::new(KucoinSenderService::from_config(config)))
        }
    }
}
