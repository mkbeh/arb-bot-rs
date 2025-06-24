use anyhow::bail;
use async_trait::async_trait;

use crate::{
    libs::{
        binance_api,
        binance_api::{General, Trade, account::Account},
    },
    services::ExchangeService,
};

pub struct BinanceService {
    account_api: Account,
    general_api: General,
    trade_api: Trade,
}

impl BinanceService {
    pub fn new(general_api: General, account_api: Account, trade_api: Trade) -> Self {
        Self {
            account_api,
            general_api,
            trade_api,
        }
    }

    async fn build_symbols_chains(&self) -> anyhow::Result<()> {
        let exchange_info = match self.general_api.exchange_info().await {
            Ok(exchange_info) => exchange_info,
            Err(err) => bail!(err),
        };

        println!("{:?}", exchange_info);

        // todo: build symbol chains

        Ok(())
    }
}

#[async_trait]
impl ExchangeService for BinanceService {
    async fn start_arbitrage(&self) -> anyhow::Result<()> {
        let chains = match self.build_symbols_chains().await {
            Ok(chains) => chains,
            Err(err) => bail!("failed to build symbols chains: {}", err),
        };

        println!("{:?}", chains);

        Ok(())
    }
}
