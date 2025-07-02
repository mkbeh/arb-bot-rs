use anyhow::bail;

use crate::{libs::binance_api::Market, services::binance::SymbolWrapper};

pub struct OrderBuilder {
    market_api: Market,
    market_depth_limit: usize,
}

impl OrderBuilder {
    pub fn new(market_api: Market, market_depth_limit: usize) -> Self {
        Self {
            market_api,
            market_depth_limit,
        }
    }

    pub async fn build_chains_orders(&self, chains: Vec<[SymbolWrapper; 3]>) -> anyhow::Result<()> {
        for chain in &chains {
            for wrapper in chain {
                let order_book = match self
                    .market_api
                    .get_depth(wrapper.symbol.symbol.clone(), self.market_depth_limit)
                    .await
                {
                    Ok(order_book) => order_book,
                    Err(e) => bail!(e),
                };

                println!("{order_book:?}");
            }
        }

        Ok(())
    }
}
