use std::sync::Arc;

use anyhow::bail;
use async_trait::async_trait;
use rust_decimal::Decimal;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;
use tracing::error;

use crate::{
    config::{Asset, Config},
    libs::{
        kucoin_api,
        kucoin_api::{BaseInfo, Kucoin, Market},
    },
    services::{
        ExchangeService,
        kucoin::exchange::{
            asset::AssetBuilder, chain::ChainBuilder, order::OrderBuilder, ticker::TickerBuilder,
        },
    },
};

pub struct KucoinExchangeConfig {
    pub base_assets: Vec<Asset>,
    pub api_url: String,
    pub min_profit_qty: Decimal,
    pub max_order_qty: Decimal,
    pub fee_percentage: Decimal,
    pub min_ticker_qty_24h: Decimal,
}

pub struct KucoinExchangeService {
    asset_builder: AssetBuilder,
    ticker_builder: TickerBuilder,
    chain_builder: Arc<ChainBuilder>,
    order_builder: Arc<OrderBuilder>,
}

impl From<&Config> for KucoinExchangeConfig {
    fn from(config: &Config) -> Self {
        Self {
            base_assets: config.settings.assets.clone(),
            api_url: config.kucoin.api_url.clone(),
            min_profit_qty: config.settings.min_profit_qty,
            max_order_qty: config.settings.max_order_qty,
            fee_percentage: config.settings.fee_percent,
            min_ticker_qty_24h: config.settings.min_ticker_qty_24h,
        }
    }
}

impl KucoinExchangeService {
    pub fn from_config(config: KucoinExchangeConfig) -> anyhow::Result<Self> {
        let api_config = kucoin_api::ClientConfig {
            host: config.api_url,
            http_config: kucoin_api::HttpConfig::default(),
        };

        let market_api: Market = match Kucoin::new(api_config.clone()) {
            Ok(client) => client,
            Err(e) => bail!("Failed init kucoin client: {e}"),
        };

        let base_info_api: BaseInfo = match Kucoin::new(api_config.clone()) {
            Ok(client) => client,
            Err(e) => bail!("Failed init kucoin client: {e}"),
        };

        let asset_builder = AssetBuilder::new(
            market_api.clone(),
            config.base_assets,
            config.min_profit_qty,
            config.max_order_qty,
            config.min_ticker_qty_24h,
        );
        let chain_builder = ChainBuilder::new(market_api.clone());
        let ticker_builder = TickerBuilder::new(base_info_api);
        let order_builder = OrderBuilder::new();

        Ok(Self {
            asset_builder,
            ticker_builder,
            chain_builder: Arc::new(chain_builder),
            order_builder: Arc::new(order_builder),
        })
    }
}

#[async_trait]
impl ExchangeService for KucoinExchangeService {
    async fn start_arbitrage(&self, token: CancellationToken) -> anyhow::Result<()> {
        let base_assets = match self.asset_builder.update_base_assets_info().await {
            Ok(assets) => assets,
            Err(e) => bail!("Failed to update base assets info: {e}"),
        };

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

        tasks_set.spawn({
            let order_builder = self.order_builder.clone();
            let token = token.clone();
            let chains = chains.clone();
            let base_assets = base_assets.clone();
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
