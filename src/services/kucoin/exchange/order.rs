use std::sync::Arc;

use itertools::Itertools;
use rust_decimal::Decimal;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;
use tracing::error;

use crate::{
    config::Asset,
    services::kucoin::{
        broadcast::TICKER_BROADCAST, exchange::chain::ChainSymbol, storage::BookTickerStore,
    },
};

pub struct OrderBuilder {
    //
}

impl OrderBuilder {
    pub fn new() -> Self {
        Self {}
    }

    pub async fn build_chains_orders(
        self: Arc<Self>,
        token: CancellationToken,
        chains: Vec<[ChainSymbol; 3]>,
        base_assets: Vec<Asset>,
    ) -> anyhow::Result<()> {
        let mut tasks_set: JoinSet<anyhow::Result<()>> = JoinSet::new();

        for chain in chains.iter() {
            tasks_set.spawn({
                let this = self.clone();
                let chain = chain.clone();
                let base_assets = base_assets.clone();
                let token = token.clone();

                async move {
                    let (mut rx1, mut rx2, mut rx3) = chain
                        .iter()
                        .map(|s| TICKER_BROADCAST.subscribe(s.symbol.symbol.as_str()))
                        .collect_tuple()
                        .expect("Invalid chain length");

                    let storage = BookTickerStore::new();
                    let last_prices: Vec<Decimal> = vec![];

                    // Read initial values from watch channel
                    {
                        _ = rx1.borrow().clone();
                        _ = rx2.borrow().clone();
                        _ = rx3.borrow().clone();
                    }

                    loop {
                        tokio::select! {
                            _ = token.cancelled() => {
                                break;
                            },

                            _ = rx1.changed() => {
                                let msg = rx1.borrow().clone();
                                // this.handle_ticker_event(&mut storage, &chain, msg, &mut last_prices, &base_assets);
                            },

                            _ = rx2.changed() => {
                                let msg = rx2.borrow().clone();
                                // this.handle_ticker_event(&mut storage, &chain, msg, &mut last_prices, &base_assets);
                            },

                            _ = rx3.changed() => {
                                let msg = rx3.borrow().clone();
                                // this.handle_ticker_event(&mut storage, &chain, msg, &mut last_prices, &base_assets);
                            },
                        }
                    }
                    Ok(())
                }
            });
        }

        while let Some(result) = tasks_set.join_next().await {
            match result {
                Ok(Err(e)) => {
                    error!(error = ?e, "Task failed");
                    token.cancel();
                }
                Err(e) => {
                    error!(error = ?e, "Join error");
                    token.cancel();
                }
                _ => {
                    token.cancel();
                }
            }
        }

        Ok(())
    }
}
