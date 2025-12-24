//! Binance exchange service module for arbitrage operations.

use std::sync::Arc;

use anyhow::Context;
use async_trait::async_trait;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;
use tracing::error;

use crate::{
    config::Config,
    libs::{
        binance_client,
        binance_client::{Binance, General, Market},
    },
    services::{
        Exchange,
        binance::exchange::{
            asset::AssetBuilder, chain::ChainBuilder, order::OrderBuilder, ticker::TickerBuilder,
        },
    },
};

/// Core service for Binance exchange arbitrage operations.
pub struct ExchangeService {
    asset_builder: AssetBuilder,
    ticker_builder: TickerBuilder,
    chain_builder: Arc<ChainBuilder>,
    order_builder: Arc<OrderBuilder>,
}

impl ExchangeService {
    pub fn from_config(config: &Config) -> anyhow::Result<Self> {
        let (settings, ex_config) = (&config.settings, &config.binance);
        let api_config = binance_client::ClientConfig {
            api_url: ex_config.api_url.clone(),
            api_token: ex_config.api_token.clone(),
            api_secret_key: ex_config.api_secret_key.clone(),
            http_config: binance_client::HttpConfig::default(),
        };

        let general_api: General =
            Binance::new(api_config.clone()).context("Failed to init general binance client")?;
        let market_api: Market =
            Binance::new(api_config).context("Failed to init market binance client")?;

        let asset_builder = AssetBuilder::new(
            market_api.clone(),
            settings.assets.clone(),
            settings.min_profit_qty,
            settings.max_order_qty,
            settings.min_ticker_qty_24h,
        );
        let ticker_builder = TickerBuilder::new(
            ex_config.ws_streams_url.clone(),
            ex_config.ws_max_connections,
        );
        let chain_builder = ChainBuilder::new(
            general_api.clone(),
            market_api.clone(),
            settings.skip_assets.clone(),
        );
        let order_builder = OrderBuilder::new(settings.market_depth_limit, settings.fee_percent);

        Ok(Self {
            asset_builder,
            ticker_builder,
            chain_builder: Arc::new(chain_builder),
            order_builder: Arc::new(order_builder),
        })
    }
}

#[async_trait]
impl Exchange for ExchangeService {
    /// Starts the arbitrage process.
    async fn start_arbitrage(&self, token: CancellationToken) -> anyhow::Result<()> {
        // Get and update base assets limits.
        let base_assets = self
            .asset_builder
            .update_base_assets_info()
            .await
            .context("Failed to update base assets info")?;

        // Get all available symbols and build chains.
        let chains = self
            .chain_builder
            .clone()
            .build_symbols_chains(base_assets.clone())
            .await
            .context("Failed to build symbols chains")?;

        let mut tasks_set = JoinSet::new();

        // Get order books per chain and calculate profit.
        tasks_set.spawn({
            let order_builder = self.order_builder.clone();
            let token = token.clone();
            let chains = chains.clone();
            async move {
                order_builder
                    .build_chains_orders(token, chains, base_assets)
                    .await
            }
        });

        // Get and update tickers order books.
        tasks_set.spawn({
            let ticker_builder = self.ticker_builder.clone();
            let token = token.clone();
            let chains = chains.clone();
            async move { ticker_builder.build_order_books(token, chains).await }
        });

        // Wait for tasks, cancel on first error
        while let Some(result) = tasks_set.join_next().await {
            match result {
                Ok(Ok(())) => {
                    token.cancel();
                    continue;
                }
                Ok(Err(e)) => {
                    error!(error = ?e, "Task failed");
                    token.cancel();
                    break;
                }
                Err(e) => {
                    error!(error = ?e, "Join error");
                    token.cancel();
                    break;
                }
            }
        }

        // Wait for the remaining tasks to complete after cancellation.
        while let Some(result) = tasks_set.join_next().await {
            if let Err(e) = result {
                error!("Task failed during shutdown: {}", e);
            }
        }

        Ok(())
    }
}
