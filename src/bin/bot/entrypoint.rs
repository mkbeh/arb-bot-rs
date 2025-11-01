use std::sync::Arc;

use anyhow::anyhow;
use app::{
    config::{Config, Exchange},
    cron::{arbitrage_job, order_sender_job},
    libs::http_server::{Server, server::ServerProcess},
    services::{
        BinanceExchangeConfig, BinanceExchangeService, BinanceSenderConfig, BinanceSenderService,
        ExchangeService, KucoinExchangeConfig, KucoinExchangeService, KucoinSenderConfig,
        KucoinSenderService, OrderSenderService, weight::REQUEST_WEIGHT,
    },
};

pub struct Entrypoint;

impl Entrypoint {
    pub async fn run(&self) -> anyhow::Result<()> {
        let config = Config::parse().map_err(|e| anyhow!("Failed to parse config file: {e}"))?;
        let settings = &config.settings;

        {
            REQUEST_WEIGHT
                .lock()
                .await
                .set_weight_limit(config.settings.api_weight_limit);
        }

        let exchange_service: Arc<dyn ExchangeService> = build_exchange_service(&config).await?;
        let sender_service: Arc<dyn OrderSenderService> = build_sender_service(&config).await?;

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

async fn build_exchange_service(config: &Config) -> anyhow::Result<Arc<dyn ExchangeService>> {
    match config.settings.exchange_name.parse()? {
        Exchange::Binance => {
            let svc = BinanceExchangeService::from_config(BinanceExchangeConfig::from(config))?;
            Ok(Arc::new(svc))
        }
        Exchange::Kucoin => {
            let svc = KucoinExchangeService::from_config(KucoinExchangeConfig::from(config))?;
            Ok(Arc::new(svc))
        }
    }
}

async fn build_sender_service(config: &Config) -> anyhow::Result<Arc<dyn OrderSenderService>> {
    match config.settings.exchange_name.parse()? {
        Exchange::Binance => {
            let svc = BinanceSenderService::from_config(BinanceSenderConfig::from(config))?;
            Ok(Arc::new(svc))
        }
        Exchange::Kucoin => {
            let svc = KucoinSenderService::from_config(KucoinSenderConfig::from(config))?;
            Ok(Arc::new(svc))
        }
    }
}
