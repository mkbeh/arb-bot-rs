use anyhow::bail;
use async_trait::async_trait;

use crate::{libs::binance_api::General, services::ExchangeService};

pub struct BinanceService {
    general_api: General,
}

impl BinanceService {
    pub fn new(general_api: General) -> Self {
        Self { general_api }
    }

    async fn get_available_assets(&self) {
        println!("Getting available assets");
    }

    async fn build_assets_chains(&self) {
        println!("Building assets chains");
    }
}

#[async_trait]
impl ExchangeService for BinanceService {
    async fn start_arbitrage(&self) -> anyhow::Result<()> {
        let exchange_info = match self.general_api.exchange_info().await {
            Ok(exchange_info) => exchange_info,
            Err(err) => bail!(err),
        };

        println!("exchange_info: {:?}", exchange_info);

        self.get_available_assets().await;
        self.build_assets_chains().await;

        Ok(())
    }
}
