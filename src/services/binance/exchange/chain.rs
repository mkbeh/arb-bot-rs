use std::{
    collections::{BTreeMap, HashMap, btree_map},
    sync::Arc,
};

use anyhow::bail;
use rust_decimal::{Decimal, prelude::Zero};
use strum::IntoEnumIterator;
use tokio::task::JoinSet;
use tracing::{debug, info};

use crate::{
    config::Asset,
    libs::binance_api::{
        General, Market, OrderType, Symbol, TickerPriceResponseType, TickerPriceStats,
    },
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
    general_api: General,
    market_api: Market,
}

impl ChainBuilder {
    pub fn new(general_api: General, market_api: Market) -> Self {
        Self {
            general_api,
            market_api,
        }
    }

    pub async fn build_symbols_chains(
        self: Arc<Self>,
        base_assets: Vec<Asset>,
    ) -> anyhow::Result<Vec<[ChainSymbol; 3]>> {
        let exchange_info = match self.general_api.exchange_info().await {
            Ok(exchange_info) => exchange_info,
            Err(e) => bail!("Failed to get exchange info: {:?}", e),
        };

        // It is necessary to launch 2 cycles of chain formation for a case where one symbol can
        // contain 2 basic assets specified in the config at once.
        let mut chains: Vec<_> = vec![];
        let mut tasks_set = JoinSet::new();

        for order in SymbolOrder::iter() {
            tasks_set.spawn({
                let this = Arc::clone(&self);
                let symbols = exchange_info.symbols.clone();
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

        let filter_chains = self
            .filter_chains_by_24h_vol(&base_assets, unique_chains)
            .await?;

        info!("Successfully build chains: {}", filter_chains.len());

        Ok(filter_chains)
    }

    async fn build_chains(
        &self,
        symbols: &[Symbol],
        order: SymbolOrder,
        base_assets: &[Asset],
    ) -> Vec<[ChainSymbol; 3]> {
        let mut chains = vec![];
        'outer_loop: for a_symbol in symbols {
            if !Self::check_order_type(&a_symbol.order_types) {
                continue 'outer_loop;
            }

            let mut a_wrapper = ChainSymbol::new(a_symbol.clone(), Default::default());
            let base_asset =
                if let Some(asset) = Self::define_base_asset(&mut a_wrapper, order, base_assets) {
                    asset
                } else {
                    continue;
                };

            for b_symbol in symbols {
                if !Self::check_order_type(&a_symbol.order_types) {
                    continue 'outer_loop;
                }

                let mut b_wrapper = ChainSymbol::new(b_symbol.clone(), Default::default());

                // Selection symbol for 1st symbol.
                if !Self::compare_symbols(&a_wrapper, &mut b_wrapper) {
                    continue;
                }

                for c_symbol in symbols {
                    if !Self::check_order_type(&a_symbol.order_types) {
                        continue 'outer_loop;
                    }

                    let mut c_wrapper = ChainSymbol::new(c_symbol.clone(), Default::default());

                    // Selection symbol for 2nd symbol.
                    if !Self::compare_symbols(&b_wrapper, &mut c_wrapper) {
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

    async fn filter_chains_by_24h_vol(
        &self,
        base_assets: &[Asset],
        chains: Vec<[ChainSymbol; 3]>,
    ) -> anyhow::Result<Vec<[ChainSymbol; 3]>> {
        let calc_volume_fn = |volume: Decimal, price: Decimal, order: SymbolOrder| -> Decimal {
            match order {
                SymbolOrder::Asc => volume * price,
                SymbolOrder::Desc => volume / price,
            }
        };

        let ticker_prices: HashMap<String, TickerPriceStats> = match self
            .market_api
            .get_ticker_price_24h::<String>(None, TickerPriceResponseType::Mini)
            .await
        {
            Ok(ticker_prices) => ticker_prices
                .into_iter()
                .map(|stats| (stats.symbol.clone(), stats))
                .collect(),
            Err(e) => bail!("failed to get ticker price: {}", e),
        };

        let mut filter_chains = vec![];
        'outer: for chain in chains {
            let mut last_volume_limit = Decimal::zero();

            for (i, chain_symbol) in chain.iter().enumerate() {
                let Some(stats) = ticker_prices.get(chain_symbol.symbol.symbol.as_str()) else {
                    continue 'outer;
                };

                let (volume, price) = match chain_symbol.order {
                    SymbolOrder::Asc => (stats.volume, stats.last_price),
                    SymbolOrder::Desc => (stats.quote_volume, stats.last_price),
                };

                if volume == Decimal::zero() || price == Decimal::zero() {
                    debug!(
                        symbol = ?chain_symbol.symbol.symbol.as_str(),
                        volume = ?volume,
                        price = ?price,
                        "skip chain ticker price",
                    );
                    continue 'outer;
                }

                match i {
                    0 => {
                        let base_asset = base_assets
                            .iter()
                            .find(|v| v.asset == Self::find_base_asset(chain_symbol))
                            .expect("base asset not found");

                        if volume < base_asset.min_ticker_qty_24h {
                            continue 'outer;
                        }

                        last_volume_limit = calc_volume_fn(
                            base_asset.min_ticker_qty_24h,
                            price,
                            chain_symbol.order,
                        );
                    }
                    _ => {
                        if volume < last_volume_limit {
                            continue 'outer;
                        }

                        last_volume_limit =
                            calc_volume_fn(last_volume_limit, price, chain_symbol.order);
                    }
                }
            }
            filter_chains.push(chain);
        }
        Ok(filter_chains)
    }

    fn check_order_type(order_types: &[OrderType]) -> bool {
        const REQUIRE_ORDER_TYPES: [OrderType; 2] = [OrderType::Limit, OrderType::Market];
        REQUIRE_ORDER_TYPES
            .iter()
            .all(|order_type| order_types.contains(order_type))
    }

    fn find_base_asset(chain_symbol: &ChainSymbol) -> String {
        match chain_symbol.order {
            // Ex: BTC:TRX
            SymbolOrder::Asc => chain_symbol.symbol.base_asset.clone(),
            // Ex: TRX:BTC -> BTC:TRX(reversed)
            SymbolOrder::Desc => chain_symbol.symbol.quote_asset.clone(),
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
                *x.asset == wrapper.symbol.base_asset || *x.asset == wrapper.symbol.quote_asset
            })
            .count();

        if base_assets_qty == MAX_ASSETS_QTY {
            wrapper.order = order;
            return Some(Self::find_base_asset(wrapper));
        }

        if base_assets
            .iter()
            .any(|x| x.asset == wrapper.symbol.base_asset.as_str())
        {
            wrapper.order = Default::default();
            return Some(Self::find_base_asset(wrapper));
        };

        if base_assets
            .iter()
            .any(|x| x.asset == wrapper.symbol.quote_asset.as_str())
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

    fn deduplicate_chains(chains: Vec<[ChainSymbol; 3]>) -> Vec<[ChainSymbol; 3]> {
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

pub fn extract_chain_symbols(chain_symbols: &[ChainSymbol]) -> Vec<&str> {
    chain_symbols
        .iter()
        .map(|v| v.symbol.symbol.as_str())
        .collect()
}
