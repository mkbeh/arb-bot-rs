use std::sync::{Arc, LazyLock};

use anyhow::bail;
use async_trait::async_trait;
use rust_decimal::Decimal;
use tokio::sync::Mutex;

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

pub static REQUEST_WEIGHT: LazyLock<Mutex<RequestWeight>> =
    LazyLock::new(|| Mutex::new(RequestWeight::default()));

pub struct BinanceConfig {
    pub account_api: Account,
    pub general_api: General,
    pub market_api: Market,
    pub trade_api: Trade,
    pub base_assets: Vec<Asset>,
    pub market_depth_limit: usize,
    pub default_min_profit_limit: Decimal,
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
    // Get and update base assets volume and profit limits from api.
    async fn update_base_assets_info(&self) -> anyhow::Result<Vec<Asset>> {
        let set_asset_volumes_fn = |asset: &Asset, stat: &TickerPriceStats| -> Asset {
            let mut new_asset = asset.clone();

            if asset.symbol.clone().unwrap().starts_with("USDT") {
                new_asset.min_profit_limit = self.default_min_profit_limit * stat.last_price;
                new_asset.max_volume_limit = self.default_max_volume_limit * stat.last_price;
            } else {
                new_asset.min_profit_limit = self.default_min_profit_limit / stat.last_price;
                new_asset.max_volume_limit = self.default_max_volume_limit / stat.last_price;
            }

            new_asset.min_profit_limit = new_asset
                .min_profit_limit
                .round_dp(new_asset.asset_precision);

            new_asset.max_volume_limit = new_asset
                .max_volume_limit
                .round_dp(new_asset.asset_precision);

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

pub struct RequestWeight {
    timestamp: u64,
    pub weight: usize,
    pub weight_limit: usize,
    pub weight_reset_secs: u64,
}

impl Default for RequestWeight {
    fn default() -> Self {
        Self::new()
    }
}

impl RequestWeight {
    pub fn new() -> Self {
        Self {
            timestamp: utils::time::get_current_timestamp(),
            weight: 0,
            weight_limit: 0,
            weight_reset_secs: 60,
        }
    }

    pub fn set_weight_limit(&mut self, weight_limit: usize) {
        self.weight_limit = weight_limit;
    }

    pub fn add(&mut self, weight: usize) -> bool {
        if (utils::time::get_current_timestamp() - self.timestamp) > self.weight_reset_secs {
            self.weight = 0
        }

        if self.weight + weight > self.weight_limit {
            return false;
        };

        self.weight += weight;
        true
    }

    pub fn sub_weight(&mut self, weight: usize) {
        if weight < self.weight {
            self.weight -= weight;
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::services::binance::RequestWeight;

    #[test]
    fn test_request_weight_add() -> anyhow::Result<()> {
        let mut request_weight = RequestWeight::new();
        request_weight.set_weight_limit(10);

        let result = request_weight.add(5);
        assert!(result);
        assert_eq!(request_weight.weight, 5);

        let result = request_weight.add(10);
        assert!(!result);
        assert_eq!(request_weight.weight, 5);

        Ok(())
    }

    #[test]
    fn test_request_weight_sub() -> anyhow::Result<()> {
        let mut request_weight = RequestWeight::new();
        request_weight.set_weight_limit(10);

        request_weight.sub_weight(5);
        assert_eq!(request_weight.weight, 0);

        let result = request_weight.add(5);
        assert!(result);
        assert_eq!(request_weight.weight, 5);

        request_weight.sub_weight(1);
        assert_eq!(request_weight.weight, 4);

        Ok(())
    }
}
