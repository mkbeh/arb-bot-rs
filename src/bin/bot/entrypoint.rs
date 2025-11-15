use std::sync::Arc;

use anyhow::Context;
use app::{
    config::{Config, Exchange, Settings},
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
        let config = Config::parse().with_context(|| "Failed to parse config file")?;

        {
            let mut weight_lock = REQUEST_WEIGHT.lock().await;
            weight_lock.set_weight_limit(config.settings.api_weight_limit);
        }

        let (exchange_service, sender_service) = build_services(&config).await?;
        let processes = build_processes(config.settings.clone(), exchange_service, sender_service)?;

        Server::new(
            config.settings.server_addr.clone(),
            config.settings.metrics_addr.clone(),
        )
        .with_processes(processes)
        .run()
        .await
        .with_context(|| "handling server error")?;

        Ok(())
    }
}

async fn build_services(
    config: &Config,
) -> anyhow::Result<(Arc<dyn ExchangeService>, Arc<dyn OrderSenderService>)> {
    let exchange = config
        .settings
        .exchange_name
        .parse::<Exchange>()
        .with_context(|| "Invalid exchange name in config")?;

    match exchange {
        Exchange::Binance => {
            let exchange_config = BinanceExchangeConfig::from(config);
            let exchange_svc = BinanceExchangeService::from_config(exchange_config)
                .with_context(|| "Failed to build Binance exchange service")?;

            let sender_config = BinanceSenderConfig::from(config);
            let sender_svc = BinanceSenderService::from_config(sender_config)
                .with_context(|| "Failed to build Binance sender service")?;

            Ok((Arc::new(exchange_svc), Arc::new(sender_svc)))
        }
        Exchange::Kucoin => {
            let exchange_config = KucoinExchangeConfig::from(config);
            let exchange_svc = KucoinExchangeService::from_config(exchange_config)
                .with_context(|| "Failed to build Kucoin exchange service")?;

            let sender_config = KucoinSenderConfig::from(config);
            let sender_svc = KucoinSenderService::from_config(sender_config)
                .with_context(|| "Failed to build Kucoin sender service")?;

            Ok((Arc::new(exchange_svc), Arc::new(sender_svc)))
        }
    }
}

fn build_processes(
    settings: Settings,
    exchange_service: Arc<dyn ExchangeService>,
    sender_service: Arc<dyn OrderSenderService>,
) -> anyhow::Result<Vec<Arc<dyn ServerProcess>>> {
    let arbitrage_config = arbitrage_job::Config::new(settings.error_timeout);
    let arbitrage_ps = arbitrage_job::Process::create(arbitrage_config, exchange_service);

    let sender_config = order_sender_job::Config::new(settings.error_timeout);
    let sender_ps = order_sender_job::Process::create(sender_config, sender_service);

    Ok(vec![arbitrage_ps, sender_ps])
}
