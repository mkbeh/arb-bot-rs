use std::sync::Arc;

use anyhow::bail;
use async_trait::async_trait;
use rust_decimal::Decimal;
use tracing::info;

use crate::{
    config::Asset,
    libs::binance_api::{General, Market},
    services::{
        ExchangeService,
        binance::exchange::{AssetBuilder, ChainBuilder, OrderBuilder},
    },
};

pub struct BinanceExchangeConfig {
    pub general_api: General,
    pub market_api: Market,
    pub base_assets: Vec<Asset>,
    pub market_depth_limit: usize,
    pub min_profit_qty: Decimal,
    pub max_order_qty: Decimal,
    pub fee_percentage: Decimal,
}

pub struct BinanceExchangeService {
    asset_builder: AssetBuilder,
    chain_builder: Arc<ChainBuilder>,
    order_builder: Arc<OrderBuilder>,
}

impl BinanceExchangeService {
    pub fn new(config: BinanceExchangeConfig) -> Self {
        let asset_builder = AssetBuilder::new(
            config.market_api.clone(),
            config.base_assets,
            config.min_profit_qty,
            config.max_order_qty,
        );
        let chain_builder = ChainBuilder::new(config.general_api);
        let order_builder = OrderBuilder::new(
            config.market_api,
            config.market_depth_limit,
            config.fee_percentage,
        );

        Self {
            asset_builder,
            chain_builder: Arc::new(chain_builder),
            order_builder: Arc::new(order_builder),
        }
    }
}

#[async_trait]
impl ExchangeService for BinanceExchangeService {
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

        // Get order books per chain and calculate profit.
        if let Err(e) = self
            .order_builder
            .clone()
            .build_chains_orders(chains, base_assets)
            .await
        {
            bail!("Failed to build chains orders: {}", e);
        }

        info!("All chains successfully passed");

        Ok(())
    }
}
