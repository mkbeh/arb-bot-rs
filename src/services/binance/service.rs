use std::sync::{Arc, LazyLock, Mutex};

use anyhow::bail;
use async_trait::async_trait;

use crate::{
    libs::{
        binance_api::{Account, General, Market, Trade},
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
    pub base_assets: Vec<String>,
    pub market_depth_limit: usize,
}

pub struct BinanceService {
    account_api: Account,
    general_api: General,
    market_api: Market,
    trade_api: Trade,

    base_assets: Vec<String>,
    market_depth_limit: usize,
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
        }
    }
}

#[async_trait]
impl ExchangeService for BinanceService {
    async fn start_arbitrage(&self) -> anyhow::Result<()> {
        let chain_builder = Arc::new(ChainBuilder::new(
            self.base_assets.clone(),
            self.general_api.clone(),
        ));
        let chains = match chain_builder.build_symbols_chains().await {
            Ok(chains) => chains,
            Err(e) => bail!("failed to build symbols chains: {}", e),
        };

        let order_builder = OrderBuilder::new(self.market_api.clone(), self.market_depth_limit);
        let chains_orders = match order_builder.build_chains_orders(chains).await {
            Ok(chains_orders) => chains_orders,
            Err(e) => bail!("failed to build chains orders: {}", e),
        };

        Ok(())
    }
}

struct RequestWeight {
    timestamp: u64,
    weight: usize,
}
