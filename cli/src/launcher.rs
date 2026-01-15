use anyhow::{Context, Result};
use engine::{Exchange, Sender, ServiceFactory, build_processes, build_services};
use tools::http::http_server::{HttpServer, HttpServerConfig};

use crate::{
    ExchangeType,
    config::{Config, GeneralConfig},
    ui,
};

pub async fn start(exchange: ExchangeType, config_path: std::path::PathBuf) -> Result<()> {
    let _cfg = match Config::load(&config_path) {
        Ok(cfg) => cfg,
        Err(e) => {
            return {
                ui::print_config_error(&config_path, &e);
                Ok(())
            };
        }
    };

    match exchange {
        ExchangeType::Binance => {
            #[cfg(feature = "binance")]
            {
                bootstrap::<binance::Provider, _>(_cfg.binance.as_ref(), &_cfg.general, exchange)
                    .await?
            }
            #[cfg(not(feature = "binance"))]
            ui::print_feature_error("binance");
        }
        ExchangeType::Kucoin => {
            #[cfg(feature = "kucoin")]
            {
                bootstrap::<kucoin::Provider, _>(_cfg.kucoin.as_ref(), &_cfg.general, exchange)
                    .await?
            }
            #[cfg(not(feature = "kucoin"))]
            ui::print_feature_error("kucoin")
        }
        ExchangeType::Solana => {
            #[cfg(feature = "solana")]
            {
                bootstrap::<solana::Provider, _>(_cfg.solana.as_ref(), &_cfg.general, exchange)
                    .await?
            }
            #[cfg(not(feature = "solana"))]
            ui::print_feature_error("solana")
        }
    }
    Ok(())
}

async fn bootstrap<P, C>(
    config: Option<&C>,
    settings: &GeneralConfig,
    exchange_type: ExchangeType,
) -> Result<()>
where
    P: ServiceFactory<dyn Exchange, Config = C> + ServiceFactory<dyn Sender, Config = C>,
{
    let config = config.ok_or_else(|| anyhow::anyhow!("{exchange_type} config not found"))?;
    let (exchange, sender) = build_services::<P, C>(config).await?;
    let processes = build_processes(exchange, sender);

    let server_config = HttpServerConfig {
        addr: settings.server_addr.clone(),
        metrics_addr: settings.metrics_addr.clone(),
        ..Default::default()
    };

    HttpServer::from_config(server_config)
        .with_processes(processes)
        .run()
        .await
        .context("HTTP Server failed")
}
