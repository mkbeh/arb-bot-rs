mod config;
mod ui;

use std::sync::Arc;

use anyhow::Context;
use clap::{Parser, Subcommand, ValueEnum};
use engine::{Exchange, Sender, ServiceFactory, build_processes, build_services};
use strum_macros::{Display, EnumIter, EnumString};
use tools::http::http_server::{HttpServer, HttpServerConfig, HttpServerProcess};

use crate::config::{Config, GeneralConfig};

#[derive(Parser)]
#[command(name = "arb-bot")]
#[command(about = ui::build_banner())]
struct Cli {
    #[command(subcommand)]
    commands: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// List available exchanges
    List,

    /// Show version
    Version,

    /// Run arbitrage bot
    Run {
        /// Exchange to use
        #[arg(short, long)]
        exchange: ExchangeType,

        /// Path to config.toml file
        #[arg(short, long)]
        config: Option<std::path::PathBuf>,
    },
}

#[derive(Debug, Clone, PartialEq, EnumString, Display, ValueEnum, EnumIter)]
pub enum ExchangeType {
    #[value(name = "binance")]
    Binance,
    #[value(name = "kucoin")]
    Kucoin,
    #[value(name = "solana")]
    Solana,
}

#[tools::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.commands {
        Commands::Version => {
            ui::print_version();
        }
        Commands::List => {
            ui::print_exchanges();
        }
        Commands::Run { exchange, config } => {
            run_bot(exchange, config).await?;
        }
    }

    Ok(())
}

async fn run_bot(exchange: ExchangeType, config: Option<std::path::PathBuf>) -> anyhow::Result<()> {
    let config_path = config.unwrap_or_else(|| "config.toml".into());
    let _config = match Config::parse(&config_path) {
        Ok(cfg) => cfg,
        Err(e) => {
            ui::print_config_error(&config_path, &e);
            return Ok(());
        }
    };

    match exchange {
        ExchangeType::Binance => {
            #[cfg(feature = "binance")]
            {
                use binance::Provider;
                bootstrap::<Provider, _>(_config.binance.as_ref(), &_config.general, exchange).await
            }
            #[cfg(not(feature = "binance"))]
            Ok(ui::print_feature_error("binance"))
        }
        ExchangeType::Kucoin => {
            #[cfg(feature = "kucoin")]
            {
                use kucoin::Provider;
                bootstrap::<Provider, _>(_config.kucoin.as_ref(), &_config.general, exchange).await
            }
            #[cfg(not(feature = "kucoin"))]
            Ok(ui::print_feature_error("kucoin"))
        }
        ExchangeType::Solana => {
            #[cfg(feature = "solana")]
            {
                use solana::Provider;
                bootstrap::<Provider, _>(_config.solana.as_ref(), &_config.general, exchange).await
            }
            #[cfg(not(feature = "solana"))]
            Ok(ui::print_feature_error("solana"))
        }
    }
}

pub async fn bootstrap<P, C>(
    config: Option<&C>,
    settings: &GeneralConfig,
    exchange_type: ExchangeType,
) -> anyhow::Result<()>
where
    P: ServiceFactory<dyn Exchange, Config = C> + ServiceFactory<dyn Sender, Config = C>,
{
    let config =
        config.ok_or_else(|| anyhow::anyhow!("{exchange_type} config not found in config.toml"))?;
    let (exchange, sender) = build_services::<P, C>(config).await?;
    let processes = build_processes(exchange, sender);
    run_http_server(settings, processes).await
}

async fn run_http_server(
    settings: &GeneralConfig,
    processes: Vec<Arc<dyn HttpServerProcess>>,
) -> anyhow::Result<()> {
    let server_config = HttpServerConfig {
        addr: settings.server_addr.clone(),
        metrics_addr: settings.metrics_addr.clone(),
        ..Default::default()
    };

    HttpServer::from_config(server_config)
        .with_processes(processes)
        .run()
        .await
        .with_context(|| "HTTP Server execution failed")?;

    Ok(())
}
