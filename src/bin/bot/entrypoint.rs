//! Entrypoint module for the arbitrage bot application.
use std::sync::Arc;

use anyhow::Context;
use app::{
    config::{Config, ExchangeType, Settings},
    cron::{arbitrage_job, sender_job},
    libs::http_server::{Server, ServerConfig, ServerProcess},
    services::{Exchange, Sender, binance, kucoin, solana_dex, weight::REQUEST_WEIGHT},
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
fn build_services(config: &Config) -> anyhow::Result<(Arc<dyn Exchange>, Arc<dyn Sender>)> {
    let exchange = config
        .settings
        .exchange_name
        .parse::<ExchangeType>()
        .with_context(|| "Invalid exchange name in config")?;

    match exchange {
        ExchangeType::Binance => {
            let ex_svc = binance::ExchangeService::from_config(&config)?;
            let sender_svc = binance::SenderService::from_config(&config)?;
            Ok((Arc::new(ex_svc), Arc::new(sender_svc)))
        }
        ExchangeType::Kucoin => {
            let ex_svc = kucoin::ExchangeService::from_config(&config)?;
            let sender_svc = kucoin::SenderService::from_config(&config)?;
            Ok((Arc::new(ex_svc), Arc::new(sender_svc)))
        }
        ExchangeType::SolanaDex => {
            let ex_svc = solana_dex::ExchangeService::from_config(&config)?;
            let sender_svc = solana_dex::SenderService::from_config(&config)?;
            Ok((Arc::new(ex_svc), Arc::new(sender_svc)))
        }
    }
}

/// Builds server processes for arbitrage and order sending jobs.
fn build_processes(
    settings: Settings,
    exchange_service: Arc<dyn Exchange>,
    sender_service: Arc<dyn Sender>,
) -> anyhow::Result<Vec<Arc<dyn ServerProcess>>> {
    let arbitrage_config = arbitrage_job::Config::new(settings.error_timeout);
    let arbitrage_ps = arbitrage_job::Process::create(arbitrage_config, exchange_service);

    let sender_config = sender_job::Config::new(settings.error_timeout);
    let sender_ps = sender_job::Process::create(sender_config, sender_service);

    Ok(vec![arbitrage_ps, sender_ps])
}
