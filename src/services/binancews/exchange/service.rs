use std::sync::Arc;

use anyhow::bail;
use async_trait::async_trait;
use futures_util::task::SpawnExt;
use rust_decimal::Decimal;
use tokio::task::JoinSet;

use crate::{
    config::Asset,
    libs::binance_api::{General, Market},
    services::{
        ExchangeService,
        binancews::{
            exchange::{
                asset::AssetBuilder, chain::ChainBuilder, order::OrderBuilder,
                ticker::TickerBuilder,
            },
            storage::BookTickerStore,
        },
    },
};

pub struct BinanceWsExchangeConfig {
    pub general_api: General,
    pub market_api: Market,
    pub base_assets: Vec<Asset>,
    pub ws_url: String,
    pub market_depth_limit: usize,
    pub min_profit_qty: Decimal,
    pub max_order_qty: Decimal,
    pub fee_percentage: Decimal,
}

pub struct BinanceWsExchangeService {
    asset_builder: AssetBuilder,
    ticker_builder: TickerBuilder,
    chain_builder: Arc<ChainBuilder>,
    order_builder: Arc<OrderBuilder>,
}

impl BinanceWsExchangeService {
    pub fn from_config(config: BinanceWsExchangeConfig) -> Self {
        let asset_builder = AssetBuilder::new(
            config.market_api.clone(),
            config.base_assets,
            config.min_profit_qty,
            config.max_order_qty,
        );
        let ticker_builder = TickerBuilder::new(config.ws_url.clone(), config.general_api.clone());
        let chain_builder = ChainBuilder::new(config.general_api.clone());
        let order_builder = OrderBuilder::new(
            config.market_api,
            config.market_depth_limit,
            config.fee_percentage,
        );

        Self {
            asset_builder,
            ticker_builder,
            chain_builder: Arc::new(chain_builder),
            order_builder: Arc::new(order_builder),
        }
    }
}

#[async_trait]
impl ExchangeService for BinanceWsExchangeService {
    async fn start_arbitrage(&self) -> anyhow::Result<()> {
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

        // Get and update tickers order books.
        let mut tasks_set = JoinSet::new();
        let store = Arc::new(BookTickerStore::new());

        tasks_set.spawn({
            let store = store.clone();
            let ticker = self.ticker_builder.clone();

            async move { ticker.build_tickers_order_books(store).await }
        });

        // Get order books per chain and calculate profit.
        if let Err(e) = self
            .order_builder
            .clone()
            .build_chains_orders(store, chains, base_assets)
            .await
        {
            bail!("Failed to build chains orders: {}", e);
        }

        while let Some(result) = tasks_set.join_next().await {
            if let Err(e) = result {
                tasks_set.abort_all();
                bail!(e)
            }
        }

        Ok(())
    }
}
