use std::sync::{Arc, LazyLock, Mutex};

use anyhow::bail;
use async_trait::async_trait;
use rust_decimal::Decimal;

use crate::{
    config::Asset,
    libs::{
        binance_api::{Account, General, Market, TickerPriceResponseType, TickerPriceStats, Trade},
        utils,
    },
    services::{
        ExchangeService,
        binance::{ChainBuilder, OrderBuilder},
    },
};

static REQUEST_WEIGHT: LazyLock<Mutex<RequestWeight>> = LazyLock::new(|| {
    Mutex::new(RequestWeight {
        timestamp: utils::time::get_current_timestamp(),
        weight: 0,
    })
});

pub struct BinanceConfig {
    pub account_api: Account,
    pub general_api: General,
    pub market_api: Market,
    pub trade_api: Trade,
    pub base_assets: Vec<Asset>,
    pub market_depth_limit: usize,
    pub default_min_profit_limit: Decimal,
    pub default_min_volume_limit: Decimal,
    pub default_max_volume_limit: Decimal,
}

pub struct BinanceService {
    account_api: Account,
    general_api: General,
    market_api: Market,
    trade_api: Trade,
    base_assets: Vec<Asset>,
    market_depth_limit: usize,
    default_min_profit_limit: Decimal,
    default_min_volume_limit: Decimal,
    default_max_volume_limit: Decimal,
}

impl BinanceService {
    pub fn new(cfg: BinanceConfig) -> Self {
        Self {
            account_api: cfg.account_api,
            general_api: cfg.general_api,
            market_api: cfg.market_api,
            trade_api: cfg.trade_api,
            base_assets: cfg.base_assets,
            market_depth_limit: cfg.market_depth_limit,
            default_min_profit_limit: cfg.default_min_profit_limit,
            default_min_volume_limit: cfg.default_min_volume_limit,
            default_max_volume_limit: cfg.default_max_volume_limit,
        }
    }
}

#[async_trait]
impl ExchangeService for BinanceService {
    async fn start_arbitrage(&self) -> anyhow::Result<()> {
        let base_assets = match self.update_base_assets_info().await {
            Ok(assets) => assets,
            Err(e) => bail!("Failed to update base assets info: {e}"),
        };

        // Get all available symbols and build chains.
        let chain_builder = Arc::new(ChainBuilder::new(
            base_assets.clone(),
            self.general_api.clone(),
        ));
        let chains = match chain_builder.build_symbols_chains().await {
            Ok(chains) => chains,
            Err(e) => bail!("failed to build symbols chains: {}", e),
        };

        // Get order books per chain and calculate profit.
        let order_builder = OrderBuilder::new(
            base_assets.clone(),
            self.market_api.clone(),
            self.market_depth_limit,
        );
        let chains_orders = match order_builder.build_chains_orders(chains).await {
            Ok(chains_orders) => chains_orders,
            Err(e) => bail!("failed to build chains orders: {}", e),
        };

        Ok(())
    }
}

impl BinanceService {
    // Get and update base assets volume and profit limits.
    async fn update_base_assets_info(&self) -> anyhow::Result<Vec<Asset>> {
        let set_asset_volumes_fn = |asset: &Asset, stat: &TickerPriceStats| -> Asset {
            let mut new_asset = asset.clone();

            if asset.symbol.clone().unwrap().starts_with("USDT") {
                new_asset.min_profit_limit = self.default_min_profit_limit * stat.last_price;
                new_asset.min_volume_limit = self.default_min_volume_limit * stat.last_price;
                new_asset.max_volume_limit = self.default_max_volume_limit * stat.last_price;
            } else {
                new_asset.min_profit_limit = self.default_min_profit_limit / stat.last_price;
                new_asset.min_volume_limit = self.default_min_volume_limit / stat.last_price;
                new_asset.max_volume_limit = self.default_max_volume_limit / stat.last_price;
            }

            new_asset
        };

        let symbols = self
            .base_assets
            .iter()
            .filter_map(|a| a.symbol.clone())
            .collect();

        let stats = self
            .market_api
            .get_ticker_price_24h(Some(symbols), TickerPriceResponseType::Mini)
            .await?;

        let mut assets = vec![];

        for asset in &self.base_assets {
            let mut found = false;
            for stat in &stats {
                if asset.symbol == Some(stat.symbol.clone()) {
                    assets.push(set_asset_volumes_fn(asset, stat));
                    found = true;
                }
            }

            if !found {
                assets.push(asset.clone());
            }
        }

        Ok(assets)
    }
}

struct RequestWeight {
    timestamp: u64,
    weight: usize,
}
