use std::{
    collections::{BTreeMap, btree_map},
    sync::Arc,
};

use anyhow::bail;
use strum::IntoEnumIterator;

use crate::{
    config::Asset,
    libs::binance_api::{General, Symbol},
    services::enums::SymbolOrder,
};

#[derive(Clone, Debug)]
pub struct ChainSymbol {
    pub symbol: Symbol,
    pub order: SymbolOrder,
}

impl ChainSymbol {
    pub fn new(symbol: Symbol, order: SymbolOrder) -> Self {
        Self { symbol, order }
    }
}

#[derive(Clone)]
pub struct ChainBuilder {
    base_assets: Vec<Asset>,
    general_api: General,
}

impl ChainBuilder {
    pub fn new(base_assets: Vec<Asset>, general_api: General) -> Self {
        Self {
            base_assets,
            general_api,
        }
    }

    pub async fn build_symbols_chains(self: Arc<Self>) -> anyhow::Result<Vec<[ChainSymbol; 3]>> {
        let exchange_info = match self.general_api.exchange_info().await {
            Ok(exchange_info) => Arc::new(exchange_info),
            Err(e) => bail!(e),
        };

        // It is necessary to launch 2 cycles of chain formation for a case where one symbol can
        // contain 2 basic assets specified in the config at once.
        let mut chains: Vec<_> = vec![];
        let mut tasks = Vec::with_capacity(SymbolOrder::iter().count());

        for order in SymbolOrder::iter() {
            tasks.push(tokio::spawn({
                let s = Arc::clone(&self);
                let info = Arc::clone(&exchange_info);
                async move { s.build_chains(info.symbols.clone(), order).await }
            }));
        }

        for task in tasks {
            chains.extend(task.await?)
        }

        Ok(self.deduplicate_chains(chains))
    }

    async fn build_chains(
        &self,
        symbols: Vec<Symbol>,
        order: SymbolOrder,
    ) -> Vec<[ChainSymbol; 3]> {
        let mut chains = Vec::new();
        for a_symbol in &symbols {
            let mut a_wrapper = ChainSymbol::new(a_symbol.clone(), Default::default());
            let base_asset = if let Some(asset) = self.define_base_asset(&mut a_wrapper, order) {
                asset
            } else {
                continue;
            };

            for b_symbol in &symbols {
                let mut b_wrapper = ChainSymbol::new(b_symbol.clone(), Default::default());

                // Selection symbol for 1st symbol.
                if !self.compare_symbols(&a_wrapper, &mut b_wrapper) {
                    continue;
                }

                for c_symbol in &symbols {
                    let mut c_wrapper = ChainSymbol::new(c_symbol.clone(), Default::default());

                    // Selection symbol for 2nd symbol.
                    if !self.compare_symbols(&b_wrapper, &mut c_wrapper) {
                        continue;
                    }

                    // Define out asset of last symbol.
                    let out_asset = if c_wrapper.order == SymbolOrder::Desc {
                        // Ex: BTC:ETH - ETH:USDT - BTC:USDT(reversed) -> base asset of
                        // last pair because reversed
                        c_symbol.base_asset.as_str()
                    } else {
                        // BTC:ETH - ETH:USDT - USDT:BTC -> quote asset of last pair
                        c_symbol.quote_asset.as_str()
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

    fn define_base_asset(&self, wrapper: &mut ChainSymbol, order: SymbolOrder) -> Option<String> {
        let get_base_asset_fn = |wrapper: &ChainSymbol| -> String {
            match wrapper.order {
                // Ex: BTC:TRX
                SymbolOrder::Asc => wrapper.symbol.base_asset.clone(),
                // Ex: TRX:BTC -> BTC:TRX(reversed)
                SymbolOrder::Desc => wrapper.symbol.quote_asset.clone(),
            }
        };

        const MAX_ASSETS_QTY: usize = 2;

        let base_assets_qty = self
            .base_assets
            .iter()
            .filter(|&x| {
                *x.asset == wrapper.symbol.base_asset || *x.asset == wrapper.symbol.quote_asset
            })
            .count();

        if base_assets_qty == MAX_ASSETS_QTY {
            wrapper.order = order;
            return Some(get_base_asset_fn(wrapper));
        }

        if self
            .base_assets
            .iter()
            .any(|x| x.asset == wrapper.symbol.base_asset.as_str())
        {
            wrapper.order = Default::default();
            return Some(get_base_asset_fn(wrapper));
        };

        if self
            .base_assets
            .iter()
            .any(|x| x.asset == wrapper.symbol.quote_asset.as_str())
        {
            wrapper.order = SymbolOrder::Desc;
            return Some(get_base_asset_fn(wrapper));
        };

        None
    }

    fn compare_symbols(&self, base: &ChainSymbol, quote: &mut ChainSymbol) -> bool {
        if base.symbol.symbol == quote.symbol.symbol {
            // Ex: BTC:USDT - BTC:USDT -> incorrect, must be skipped.
            return false;
        }

        match base.order {
            SymbolOrder::Asc => {
                // Ex: USDT:BTC - BTC:ETH -> valid
                if base.symbol.quote_asset == quote.symbol.base_asset {
                    return true;
                }

                // Ex: USDT:BTC - ETH:BTC -> USDT:BTC - BTC:ETH(reversed) -> valid
                if base.symbol.quote_asset == quote.symbol.quote_asset {
                    quote.order = SymbolOrder::Desc;
                    return true;
                }
            }
            SymbolOrder::Desc => {
                // Ex: BTC:USDT - BTC:ETH -> USDT:BTC(reversed) - BTC:ETH -> valid
                if base.symbol.base_asset == quote.symbol.base_asset {
                    return true;
                }

                // Ex: BTC:USDT - ETH:BTC -> USDT:BTC(reversed) - BTC:ETH(reversed) -> valid
                if base.symbol.base_asset == quote.symbol.quote_asset {
                    quote.order = SymbolOrder::Desc;
                    return true;
                }
            }
        }

        false
    }

    fn deduplicate_chains(&self, chains: Vec<[ChainSymbol; 3]>) -> Vec<[ChainSymbol; 3]> {
        let mut m: BTreeMap<String, bool> = BTreeMap::new();
        let mut unique_chains: Vec<[ChainSymbol; 3]> = Vec::new();

        let define_symbol = |x: &ChainSymbol| -> String {
            match x.order {
                SymbolOrder::Asc => x.symbol.symbol.to_string(),
                SymbolOrder::Desc => format!("{}{}", x.symbol.quote_asset, x.symbol.base_asset),
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
