use std::{
    collections::{BTreeMap, btree_map},
    sync::Arc,
};

use anyhow::bail;
use strum::IntoEnumIterator;
use tokio::task::JoinSet;
use tracing::info;

use crate::{
    config::Asset,
    libs::kucoin_api::{Market, models::Symbol},
    services::enums::SymbolOrder,
};

#[derive(Clone, Debug)]
pub struct ChainSymbol {
    pub symbol: Symbol,
    pub order: SymbolOrder,
}

#[derive(Clone)]
pub struct ChainBuilder {
    market_api: Market,
}

impl ChainSymbol {
    pub fn new(symbol: Symbol, order: SymbolOrder) -> Self {
        Self { symbol, order }
    }
}

impl ChainBuilder {
    pub fn new(market_api: Market) -> Self {
        Self { market_api }
    }

    pub async fn build_symbols_chains(
        self: Arc<Self>,
        base_assets: Vec<Asset>,
    ) -> anyhow::Result<Vec<[ChainSymbol; 3]>> {
        let symbols_response = match self.market_api.get_all_symbols(None).await {
            Ok(response) => response,
            Err(e) => bail!(e),
        };

        let mut chains: Vec<_> = vec![];
        let mut tasks_set = JoinSet::new();

        for order in SymbolOrder::iter() {
            tasks_set.spawn({
                let this = Arc::clone(&self);
                let symbols = symbols_response.data.clone();
                let assets = base_assets.clone();
                async move { this.build_chains(&symbols, order, &assets).await }
            });
        }

        while let Some(result) = tasks_set.join_next().await {
            match result {
                Ok(chain) => chains.extend(chain),
                Err(e) => bail!(e),
            }
        }

        let unique_chains = Self::deduplicate_chains(chains);

        info!("🚀 Successfully build chains: {}", unique_chains.len());

        Ok(unique_chains)
    }

    async fn build_chains(
        &self,
        symbols: &[Symbol],
        order: SymbolOrder,
        base_assets: &[Asset],
    ) -> Vec<[ChainSymbol; 3]> {
        let mut chains = vec![];
        for a_symbol in symbols {
            let mut a_wrapper = ChainSymbol::new(a_symbol.clone(), Default::default());
            let base_asset =
                if let Some(asset) = Self::define_base_asset(&mut a_wrapper, order, base_assets) {
                    asset
                } else {
                    continue;
                };

            for b_symbol in symbols {
                let mut b_wrapper = ChainSymbol::new(b_symbol.clone(), Default::default());

                // Selection symbol for 1st symbol.
                if !Self::compare_symbols(&a_wrapper, &mut b_wrapper) {
                    continue;
                }

                for c_symbol in symbols {
                    let mut c_wrapper = ChainSymbol::new(c_symbol.clone(), Default::default());

                    // Selection symbol for 2nd symbol.
                    if !Self::compare_symbols(&b_wrapper, &mut c_wrapper) {
                        continue;
                    }

                    // Define out asset of last symbol.
                    let out_asset = if c_wrapper.order == SymbolOrder::Desc {
                        // Ex: BTC:ETH - ETH:USDT - BTC:USDT(reversed) -> base asset of
                        // last pair because reversed
                        c_symbol.base_currency.as_str()
                    } else {
                        // BTC:ETH - ETH:USDT - USDT:BTC -> quote asset of last pair
                        c_symbol.quote_currency.as_str()
                    };

                    // Exit from 3rd symbol must be into base asset from the 1st symbol.
                    if base_asset != out_asset {
                        continue;
                    }

                    chains.push([a_wrapper.clone(), b_wrapper.clone(), c_wrapper.clone()]);
                }
            }
        }
        chains
    }

    fn find_base_asset(chain_symbol: &ChainSymbol) -> String {
        match chain_symbol.order {
            // Ex: BTC:TRX
            SymbolOrder::Asc => chain_symbol.symbol.base_currency.clone(),
            // Ex: TRX:BTC -> BTC:TRX(reversed)
            SymbolOrder::Desc => chain_symbol.symbol.quote_currency.clone(),
        }
    }

    fn define_base_asset(
        wrapper: &mut ChainSymbol,
        order: SymbolOrder,
        base_assets: &[Asset],
    ) -> Option<String> {
        const MAX_ASSETS_QTY: usize = 2;

        let base_assets_qty = base_assets
            .iter()
            .filter(|&x| {
                *x.asset == wrapper.symbol.base_currency
                    || *x.asset == wrapper.symbol.quote_currency
            })
            .count();

        if base_assets_qty == MAX_ASSETS_QTY {
            wrapper.order = order;
            return Some(Self::find_base_asset(wrapper));
        }

        if base_assets
            .iter()
            .any(|x| x.asset == wrapper.symbol.base_currency.as_str())
        {
            wrapper.order = Default::default();
            return Some(Self::find_base_asset(wrapper));
        };

        if base_assets
            .iter()
            .any(|x| x.asset == wrapper.symbol.quote_currency.as_str())
        {
            wrapper.order = SymbolOrder::Desc;
            return Some(Self::find_base_asset(wrapper));
        };

        None
    }

    fn compare_symbols(base: &ChainSymbol, quote: &mut ChainSymbol) -> bool {
        if base.symbol.symbol == quote.symbol.symbol {
            // Ex: BTC:USDT - BTC:USDT -> incorrect, must be skipped.
            return false;
        }

        match base.order {
            SymbolOrder::Asc => {
                // Ex: USDT:BTC - BTC:ETH -> valid
                if base.symbol.quote_currency == quote.symbol.base_currency {
                    return true;
                }

                // Ex: USDT:BTC - ETH:BTC -> USDT:BTC - BTC:ETH(reversed) -> valid
                if base.symbol.quote_currency == quote.symbol.quote_currency {
                    quote.order = SymbolOrder::Desc;
                    return true;
                }
            }
            SymbolOrder::Desc => {
                // Ex: BTC:USDT - BTC:ETH -> USDT:BTC(reversed) - BTC:ETH -> valid
                if base.symbol.base_currency == quote.symbol.base_currency {
                    return true;
                }

                // Ex: BTC:USDT - ETH:BTC -> USDT:BTC(reversed) - BTC:ETH(reversed) -> valid
                if base.symbol.base_currency == quote.symbol.quote_currency {
                    quote.order = SymbolOrder::Desc;
                    return true;
                }
            }
        }
        false
    }

    fn deduplicate_chains(chains: Vec<[ChainSymbol; 3]>) -> Vec<[ChainSymbol; 3]> {
        let mut m: BTreeMap<String, bool> = BTreeMap::new();
        let mut unique_chains: Vec<[ChainSymbol; 3]> = Vec::new();

        let define_symbol = |x: &ChainSymbol| -> String {
            match x.order {
                SymbolOrder::Asc => x.symbol.symbol.to_string(),
                SymbolOrder::Desc => {
                    format!("{}{}", x.symbol.quote_currency, x.symbol.base_currency)
                }
            }
        };

        for chain in chains.iter() {
            let key = format!(
                "{}({}):{}({}):{}({})",
                define_symbol(&chain[0]),
                &chain[0].order,
                define_symbol(&chain[1]),
                &chain[0].order,
                define_symbol(&chain[2]),
                &chain[0].order,
            );

            if let btree_map::Entry::Vacant(e) = m.entry(key) {
                e.insert(true);
                unique_chains.push(chain.clone());
            }
        }

        unique_chains
    }
}

pub fn extract_chain_symbols(chain_symbols: &[ChainSymbol]) -> Vec<&str> {
    chain_symbols
        .iter()
        .map(|v| v.symbol.symbol.as_str())
        .collect()
}
