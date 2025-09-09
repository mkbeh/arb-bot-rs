use std::sync::Arc;

use anyhow::bail;
use async_trait::async_trait;
use rust_decimal::Decimal;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;
use tracing::error;

use crate::{
    config::Asset,
    libs::binance_api::{General, Market},
    services::{
        ExchangeService,
        binance::exchange::{
            asset::AssetBuilder, chain::ChainBuilder, order::OrderBuilder, ticker::TickerBuilder,
        },
    },
};

pub struct BinanceExchangeConfig {
    pub general_api: General,
    pub market_api: Market,
    pub base_assets: Vec<Asset>,
    pub ws_streams_url: String,
    pub ws_max_connections: usize,
    pub market_depth_limit: usize,
    pub min_profit_qty: Decimal,
    pub max_order_qty: Decimal,
    pub fee_percentage: Decimal,
    pub min_ticker_qty_24h: Decimal,
}

pub struct BinanceExchangeService {
    asset_builder: AssetBuilder,
    ticker_builder: TickerBuilder,
    chain_builder: Arc<ChainBuilder>,
    order_builder: Arc<OrderBuilder>,
}

impl BinanceExchangeService {
    pub fn from_config(config: BinanceExchangeConfig) -> Self {
        let asset_builder = AssetBuilder::new(
            config.market_api.clone(),
            config.base_assets,
            config.min_profit_qty,
            config.max_order_qty,
            config.min_ticker_qty_24h,
        );

        let ticker_builder =
            TickerBuilder::new(config.ws_streams_url.clone(), config.ws_max_connections);

        let chain_builder =
            ChainBuilder::new(config.general_api.clone(), config.market_api.clone());

        let order_builder = OrderBuilder::new(config.market_depth_limit, config.fee_percentage);

        Self {
            asset_builder,
            ticker_builder,
            chain_builder: Arc::new(chain_builder),
            order_builder: Arc::new(order_builder),
        }
    }
}

#[async_trait]
impl ExchangeService for BinanceExchangeService {
    async fn start_arbitrage(&self, token: CancellationToken) -> anyhow::Result<()> {
        // Get and update base assets limits.
        let base_assets = match self.asset_builder.update_base_assets_info().await {
            Ok(assets) => assets,
            Err(e) => bail!("Failed to update base assets info: {e}"),
        };

        // Get all available symbols and build chains.
        let chains = match self
            .chain_builder
            .clone()
            .build_symbols_chains(base_assets.clone())
            .await
        {
            Ok(chains) => chains,
            Err(e) => bail!("failed to build symbols chains: {}", e),
        };

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

        loop {
            tokio::select! {
                _ = token.cancelled() => {
                    break;
                }
                result = tasks_set.join_next() => match result {
                    Some(Ok(Err(e))) => {
                        error!(error = ?e, "Failed to run task");
                        token.cancel();
                        break;
                    }
                    Some(Err(e)) => {
                        error!(error = ?e, "Failed to join task");
                        token.cancel();
                        break;
                    }
                    _ => {
                        token.cancel();
                        continue;
                    }
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
