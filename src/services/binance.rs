use std::collections::{BTreeMap, btree_map};

use anyhow::bail;
use async_trait::async_trait;
use strum::IntoEnumIterator;

use crate::{
    libs::binance_api::{Account, General, Market, Symbol, Trade},
    services::{ExchangeService, enums::SymbolOrder},
};

pub struct BinanceService {
    base_assets: Vec<String>,
    account_api: Account,
    general_api: General,
    market_api: Market,
    trade_api: Trade,
}

impl BinanceService {
    pub fn new(
        base_assets: Vec<String>,
        account_api: Account,
        general_api: General,
        market_api: Market,
        trade_api: Trade,
    ) -> Self {
        Self {
            base_assets,
            account_api,
            general_api,
            market_api,
            trade_api,
        }
    }
}

#[async_trait]
impl ExchangeService for BinanceService {
    async fn start_arbitrage(&self) -> anyhow::Result<()> {
        let chains_builder = ChainsBuilder::new(self.base_assets.clone(), self.general_api.clone());
        let chains = match chains_builder.build_symbols_chains().await {
            Ok(chains) => chains,
            Err(err) => bail!("failed to build symbols chains: {}", err),
        };

        println!("{:?}", chains);

        Ok(())
    }
}

#[derive(Clone, Debug)]
struct SymbolWrapper {
    symbol: Symbol,
    order: SymbolOrder,
}

impl SymbolWrapper {
    fn new(symbol: Symbol, order: SymbolOrder) -> Self {
        Self { symbol, order }
    }
}

#[derive(Clone)]
struct ChainsBuilder {
    base_assets: Vec<String>,
    general_api: General,
}

impl ChainsBuilder {
    fn new(base_assets: Vec<String>, general_api: General) -> Self {
        Self {
            base_assets,
            general_api,
        }
    }

    async fn build_symbols_chains(&self) -> anyhow::Result<Vec<[SymbolWrapper; 3]>> {
        let exchange_info = match self.general_api.exchange_info().await {
            Ok(exchange_info) => exchange_info,
            Err(err) => bail!(err),
        };

        let build_chains = |order: SymbolOrder| -> Vec<[SymbolWrapper; 3]> {
            let mut chains = Vec::new();
            for a_symbol in exchange_info.symbols.iter() {
                let mut a_wrapper = SymbolWrapper::new(a_symbol.clone(), Default::default());
                let base_asset = if let Some(asset) = self.define_base_asset(&mut a_wrapper, order)
                {
                    asset
                } else {
                    continue;
                };

                for b_symbol in exchange_info.symbols.iter() {
                    let mut b_wrapper = SymbolWrapper::new(b_symbol.clone(), Default::default());

                    // Selection symbol for 1st symbol.
                    if !self.compare_symbols(&a_wrapper, &mut b_wrapper) {
                        continue;
                    }

                    for c_symbol in exchange_info.symbols.iter() {
                        let mut c_wrapper =
                            SymbolWrapper::new(c_symbol.clone(), Default::default());

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
        };

        // It is necessary to launch 2 cycles of chain formation for a case where one symbol can
        // contain 2 basic assets specified in the config at once.
        let mut chains = vec![];
        for v in SymbolOrder::iter() {
            chains.extend(build_chains(v));
        }

        Ok(self.deduplicate_chains(chains))
    }

    fn define_base_asset(&self, wrapper: &mut SymbolWrapper, order: SymbolOrder) -> Option<String> {
        let get_base_asset = |wrapper: &SymbolWrapper| -> String {
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
            .filter(|&x| *x == wrapper.symbol.base_asset || *x == wrapper.symbol.quote_asset)
            .count();

        if base_assets_qty == MAX_ASSETS_QTY {
            wrapper.order = order;
            return Some(get_base_asset(wrapper));
        }

        if self
            .base_assets
            .iter()
            .any(|x| x == wrapper.symbol.base_asset.as_str())
        {
            wrapper.order = Default::default();
            return Some(get_base_asset(wrapper));
        };

        if self
            .base_assets
            .iter()
            .any(|x| x == wrapper.symbol.quote_asset.as_str())
        {
            wrapper.order = SymbolOrder::Desc;
            return Some(get_base_asset(wrapper));
        };

        None
    }

    fn compare_symbols(&self, base: &SymbolWrapper, quote: &mut SymbolWrapper) -> bool {
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

    fn deduplicate_chains(&self, chains: Vec<[SymbolWrapper; 3]>) -> Vec<[SymbolWrapper; 3]> {
        let mut m: BTreeMap<String, bool> = BTreeMap::new();
        let mut unique_chains: Vec<[SymbolWrapper; 3]> = Vec::new();

        let define_symbol = |x: &SymbolWrapper| -> String {
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

            // if !m.contains_key(&key) {
            //     m.insert(key, true);
            //     unique_chains.push(chain.clone());
            // }
        }

        unique_chains
    }
}
