//! Entrypoint module for the arbitrage bot application.
use std::sync::Arc;

use anyhow::Context;
use app::{
    config::{Config, Exchange, Settings},
    cron::{arbitrage_job, order_sender_job},
    libs::http_server::{Server, ServerConfig, server::ServerProcess},
    services::{
        BinanceExchangeConfig, BinanceExchangeService, BinanceSenderConfig, BinanceSenderService,
        ExchangeService, KucoinExchangeConfig, KucoinExchangeService, KucoinSenderConfig,
        KucoinSenderService, OrderSenderService, weight::REQUEST_WEIGHT,
    },
};

/// Main entrypoint struct for the application.
///
/// This is a zero-sized struct used as a singleton to invoke the application's
/// runtime logic. It handles the full lifecycle from config loading to server startup.
pub struct Entrypoint;

impl Entrypoint {
    /// Runs the application, performing all necessary setup and starting the server.
    pub async fn run(&self) -> anyhow::Result<()> {
        let config = Config::parse().with_context(|| "Failed to parse config file")?;

        // Configure global request weight limit for API rate limiting.
        {
            let mut weight_lock = REQUEST_WEIGHT.lock().await;
            weight_lock.set_weight_limit(config.settings.api_weight_limit);
        }

        // Build trait-object services for exchange and order sending.
        let (exchange_service, sender_service) = build_services(&config)?;

        // Create server processes for cron-like jobs
        let processes = build_processes(config.settings.clone(), exchange_service, sender_service)?;

        // Initialize and run the HTTP server with processes and metrics.
        self.run_http_server(&config.settings, processes).await?;

        Ok(())
    }

    /// Runs the HTTP server with the given config and processes.
    async fn run_http_server(
        &self,
        settings: &Settings,
        processes: Vec<Arc<dyn ServerProcess>>,
    ) -> anyhow::Result<()> {
        let server_config = ServerConfig {
            addr: settings.server_addr.clone(),
            metrics_addr: settings.metrics_addr.clone(),
            ..ServerConfig::default()
        };

        Server::from_config(server_config)
            .with_processes(processes)
            .run()
            .await
            .with_context(|| "handling server error")?;

        Ok(())
    }
}

/// Builds exchange-specific services based on the configuration.
fn build_services(
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

/// Builds server processes for arbitrage and order sending jobs.
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
