//! Binance exchange service module for arbitrage operations.

use std::sync::Arc;

use anyhow::Context;
use async_trait::async_trait;
use rust_decimal::Decimal;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;
use tracing::error;

use crate::{
    config::{Asset, Config},
    libs::{
        binance_api,
        binance_api::{Binance, General, Market},
    },
    services::{
        Exchange,
        binance::exchange::{
            asset::AssetBuilder, chain::ChainBuilder, order::OrderBuilder, ticker::TickerBuilder,
        },
    },
};

/// Configuration for the Binance exchange service.
pub struct ExchangeConfig {
    pub api_url: String,
    pub api_token: String,
    pub api_secret_key: String,
    pub base_assets: Vec<Asset>,
    pub ws_streams_url: String,
    pub ws_max_connections: usize,
    pub market_depth_limit: usize,
    pub min_profit_qty: Decimal,
    pub max_order_qty: Decimal,
    pub fee_percentage: Decimal,
    pub min_ticker_qty_24h: Decimal,
    pub skip_assets: Vec<String>,
}

impl From<&Config> for ExchangeConfig {
    fn from(config: &Config) -> Self {
        Self {
            api_url: config.binance.api_url.clone(),
            api_token: config.binance.api_token.clone(),
            api_secret_key: config.binance.api_secret_key.clone(),
            base_assets: config.settings.assets.clone(),
            ws_streams_url: config.binance.ws_streams_url.clone(),
            ws_max_connections: config.binance.ws_max_connections,
            market_depth_limit: config.settings.market_depth_limit,
            min_profit_qty: config.settings.min_profit_qty,
            max_order_qty: config.settings.max_order_qty,
            fee_percentage: config.settings.fee_percent,
            min_ticker_qty_24h: config.settings.min_ticker_qty_24h,
            skip_assets: config.settings.skip_assets.clone(),
        }
    }
}

/// Core service for Binance exchange arbitrage operations.
pub struct ExchangeService {
    asset_builder: AssetBuilder,
    ticker_builder: TickerBuilder,
    chain_builder: Arc<ChainBuilder>,
    order_builder: Arc<OrderBuilder>,
}

impl ExchangeService {
    pub fn from_config(config: ExchangeConfig) -> anyhow::Result<Self> {
        let api_config = binance_api::ClientConfig {
            api_url: config.api_url,
            api_token: config.api_token,
            api_secret_key: config.api_secret_key,
            http_config: binance_api::HttpConfig::default(),
        };

        let general_api: General =
            Binance::new(api_config.clone()).context("Failed to init general Binance client")?;
        let market_api: Market =
            Binance::new(api_config).context("Failed to init market Binance client")?;

        let asset_builder = AssetBuilder::new(
            market_api.clone(),
            config.base_assets,
            config.min_profit_qty,
            config.max_order_qty,
            config.min_ticker_qty_24h,
        );
        let ticker_builder =
            TickerBuilder::new(config.ws_streams_url.clone(), config.ws_max_connections);
        let chain_builder =
            ChainBuilder::new(general_api.clone(), market_api.clone(), config.skip_assets);
        let order_builder = OrderBuilder::new(config.market_depth_limit, config.fee_percentage);

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
