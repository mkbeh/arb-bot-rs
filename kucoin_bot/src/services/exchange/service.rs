//! Kucoin exchange service module for arbitrage operations.

use std::sync::Arc;

use anyhow::Context;
use async_trait::async_trait;
use engine::{Exchange, REQUEST_WEIGHT, service::traits::ArbitrageService};
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;
use tracing::error;

use crate::{
    Config,
    libs::{
        kucoin_client,
        kucoin_client::{BaseInfo, Kucoin, Market},
    },
    services::exchange::{
        asset::AssetBuilder, chain::ChainBuilder, order::OrderBuilder, ticker::TickerBuilder,
    },
};

/// Core service for exchange arbitrage operations.
pub struct ExchangeService {
    asset_builder: AssetBuilder,
    ticker_builder: TickerBuilder,
    chain_builder: Arc<ChainBuilder>,
    order_builder: Arc<OrderBuilder>,
}

impl ExchangeService {
    pub async fn from_config(config: &Config) -> anyhow::Result<Self> {
        let api_config = kucoin_client::ClientConfig {
            host: config.api_url.clone(),
            api_key: config.api_token.clone(),
            api_secret: config.api_secret_key.clone(),
            api_passphrase: config.api_passphrase.clone(),
            http_config: kucoin_client::HttpConfig::default(),
        };

        let market_api: Market =
            Kucoin::new(api_config.clone()).context("Failed to init market Kucoin client")?;
        let base_info_api: BaseInfo =
            Kucoin::new(api_config).context("Failed to init base info Kucoin client")?;

        // Configure global request weight limit for API rate limiting.
        {
            let mut weight_lock = REQUEST_WEIGHT.lock().await;
            weight_lock.set_weight_limit(config.api_weight_limit);
        }

        Ok(Self {
            asset_builder: AssetBuilder::new(
                market_api.clone(),
                config.assets.clone(),
                config.min_profit_qty,
                config.max_order_qty,
                config.min_ticker_qty_24h,
            ),
            ticker_builder: TickerBuilder::new(base_info_api),
            chain_builder: Arc::new(ChainBuilder::new(
                market_api.clone(),
                config.skip_assets.clone(),
            )),
            order_builder: Arc::new(OrderBuilder::new(config.fee_percent)),
        })
    }
}

impl Exchange for ExchangeService {}

#[async_trait]
impl ArbitrageService for ExchangeService {
    /// Starts the arbitrage process.
    async fn start(&self, token: CancellationToken) -> anyhow::Result<()> {
        // Update base assets limits
        let base_assets = self
            .asset_builder
            .update_base_assets_info()
            .await
            .context("Failed to update base assets info")?;

        // Build all available symbols and chains
        let chains = self
            .chain_builder
            .clone()
            .build_symbols_chains(base_assets.clone())
            .await
            .context("Failed to build symbols chains")?;

        let mut tasks_set = JoinSet::new();

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
