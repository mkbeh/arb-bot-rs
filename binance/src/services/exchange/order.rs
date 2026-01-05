//! Order builder module for arbitrage chain processing and profit calculation.
//!
//! This module provides an `OrderBuilder` for monitoring ticker updates in triangular chains,
//! calculating potential arbitrage profits by simulating order fills (considering depth, fees,
//! filters), and generating executable `Order` chains when thresholds are met. It uses broadcast
//! channels for real-time events, scales quantities by precision/tick sizes, and propagates qty
//! limits across the chain. Supports Asc/Desc symbol orders with lot/tick filters from exchange
//! info.

use std::{ops::Sub, sync::Arc};

use engine::{ChainOrder, ChainOrders, METRICS, ORDERS_CHANNEL, enums::SymbolOrder};
use itertools::Itertools;
use rust_decimal::{
    Decimal,
    prelude::{FromPrimitive, Zero},
};
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;
use tools::misc;
use tracing::error;
use uuid::Uuid;

use crate::{
    config::Asset,
    libs::binance_client::Filters,
    services::{
        broadcast::TICKER_BROADCAST,
        exchange::{chain, chain::ChainSymbol},
        storage::{BookTickerEvent, BookTickerStore},
    },
};

/// Symbol wrapper for order building with precision, limits, and current ticker.
#[derive(Clone, Debug)]
pub struct OrderSymbol<'a> {
    pub symbol: String,
    pub base_asset_precision: u32,
    pub quote_precision: u32,
    pub symbol_order: SymbolOrder,
    pub min_profit_qty: Option<Decimal>,
    pub max_order_qty: Option<Decimal>,
    pub order_book: &'a BookTickerEvent,
    pub symbol_filter: SymbolFilter,
}

/// Intermediate order structure during chain qty/profit calculation.
#[derive(Clone, Debug)]
pub struct PreOrder {
    symbol: String,
    symbol_order: SymbolOrder,
    price: Decimal,
    base_qty: Decimal,
    base_precision: u32,
    quote_qty: Decimal,
    quote_precision: u32,
    symbol_filter: SymbolFilter,
}

/// Symbol filter structure from exchange info.
#[derive(Clone, Debug, Default)]
pub struct SymbolFilter {
    pub lot_size_step: u32,
    pub tick_size: u32,
    pub lot_size_min_qty: Decimal,
}

pub struct OrderBookUnit {
    pub price: Decimal,
    pub qty: Decimal,
}

/// Builder for processing arbitrage chains and generating profitable orders.
pub struct OrderBuilder {
    market_depth_limit: usize,
    fee_percent: Decimal,
}

impl OrderBuilder {
    #[must_use]
    pub fn new(fee_percent: Decimal) -> Self {
        Self {
            market_depth_limit: 1, // always 1
            fee_percent,
        }
    }

    /// Builds and monitors order processing tasks for the given chains.
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

                    let mut storage = BookTickerStore::new();
                    let mut last_prices: Vec<Decimal> = vec![];

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
                                this.handle_ticker_event(&mut storage, &chain, msg, &mut last_prices, &base_assets);
                            },

                            _ = rx2.changed() => {
                                let msg = rx2.borrow().clone();
                                this.handle_ticker_event(&mut storage, &chain, msg, &mut last_prices, &base_assets);
                            },

                            _ = rx3.changed() => {
                                let msg = rx3.borrow().clone();
                                this.handle_ticker_event(&mut storage, &chain, msg, &mut last_prices, &base_assets);
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

    /// Handles a ticker event update for a chain.
    pub fn handle_ticker_event(
        &self,
        storage: &mut BookTickerStore,
        chain: &[ChainSymbol; 3],
        msg: BookTickerEvent,
        last_prices: &mut Vec<Decimal>,
        base_assets: &[Asset],
    ) {
        storage.update(msg);

        // Early return if not all data is available
        let messages: Vec<BookTickerEvent> = chain
            .iter()
            .filter_map(|symbol| storage.get(symbol.symbol.symbol.as_str()).cloned())
            .collect();

        if messages.len() != chain.len() {
            return;
        }

        // Calculate prices
        let prices = chain
            .iter()
            .zip(&messages)
            .map(|(symbol, message)| match symbol.order {
                SymbolOrder::Asc => message.bid_price,
                SymbolOrder::Desc => message.ask_price,
            })
            .collect::<Vec<Decimal>>();

        // Skip if prices haven't changed
        if *last_prices == prices {
            return;
        }

        *last_prices = prices;

        // Process the chain
        if let Err(e) = Self::process_chain(
            base_assets,
            chain,
            &messages,
            self.market_depth_limit,
            self.fee_percent,
        ) {
            error!(error = ?e, "Error during process arbitrage");
        }
    }

    /// Builds orders for the chain and calculates profit.
    pub fn process_chain(
        base_assets: &[Asset],
        chain: &[ChainSymbol; 3],
        order_book: &[BookTickerEvent],
        market_depth_limit: usize,
        fee_percent: Decimal,
    ) -> anyhow::Result<()> {
        let mut order_symbols = vec![];

        for (i, chain_symbol) in chain.iter().enumerate() {
            // Define limits for 1st pair.
            let min_profit_qty = if i == 0 {
                find_base_asset(base_assets, chain_symbol).map(|base| base.min_profit_qty)
            } else {
                None
            };

            let max_order_qty = if i == 0 {
                find_base_asset(base_assets, chain_symbol).map(|base| base.max_order_qty)
            } else {
                None
            };

            let symbol = &chain_symbol.symbol;
            let order_symbol = OrderSymbol {
                symbol: symbol.symbol.clone(),
                base_asset_precision: symbol.base_asset_precision,
                quote_precision: symbol.quote_precision,
                symbol_order: chain_symbol.order,
                min_profit_qty,
                max_order_qty,
                order_book: &order_book[i],
                symbol_filter: define_symbol_filter(&symbol.filters),
            };
            order_symbols.push(order_symbol);
        }

        let orders = Self::calculate_chain_profit(&order_symbols, market_depth_limit, fee_percent);
        METRICS.record_processed_chain(&chain::extract_chain_symbols(chain));

        if orders.is_empty() {
            return Ok(());
        }

        let chain_orders = ChainOrders {
            ts: misc::time::get_current_timestamp().as_millis(),
            chain_id: Uuid::new_v4(),
            fee_percent,
            orders,
        };

        if let Err(e) = ORDERS_CHANNEL.tx.send(chain_orders) {
            error!(error = ?e, "Failed to send chain to channel");
        }

        Ok(())
    }

    /// Builds orders for the chain and calculates profit.
    #[must_use]
    pub fn calculate_chain_profit(
        chain: &[OrderSymbol],
        market_depth_limit: usize,
        fee_percent: Decimal,
    ) -> Vec<ChainOrder> {
        let mut orders: Vec<PreOrder> = vec![];
        let mut start_depth_limit = 0;

        // Extract max order qty from first symbol in the chain.
        let max_order_qty = get_max_order_qty(chain.first().unwrap());

        while start_depth_limit < market_depth_limit {
            for (i, order_symbol) in chain.iter().enumerate() {
                // Define list of orders according to the order of assets in symbol.
                let order_units: &Vec<OrderBookUnit> = match order_symbol.symbol_order {
                    SymbolOrder::Asc => &vec![OrderBookUnit {
                        price: order_symbol.order_book.bid_price,
                        qty: order_symbol.order_book.bid_qty,
                    }],
                    SymbolOrder::Desc => &vec![OrderBookUnit {
                        price: order_symbol.order_book.ask_price,
                        qty: order_symbol.order_book.ask_qty,
                    }],
                };

                // Define qty limit for current symbol.
                let max_order_qty = if i == 0 {
                    max_order_qty
                } else {
                    orders[orders.len() - 1].quote_qty
                };

                // Sum orders qty based on current depth.
                // When summing up the qty, the last price for the entire qty is taken.
                let mut price = Decimal::zero();
                let mut base_qty = Decimal::zero();

                for order_unit in order_units.iter().take(start_depth_limit + 1) {
                    let qty = match order_symbol.symbol_order {
                        SymbolOrder::Asc => order_unit.qty,
                        SymbolOrder::Desc => (order_unit.qty * order_unit.price)
                            .trunc_with_scale(order_symbol.quote_precision),
                    };

                    price = order_unit.price;
                    base_qty += qty;

                    if base_qty >= max_order_qty {
                        base_qty = max_order_qty;
                        break;
                    }
                }

                let quote_qty = match order_symbol.symbol_order {
                    SymbolOrder::Asc => {
                        (base_qty * price).trunc_with_scale(order_symbol.quote_precision)
                    }
                    SymbolOrder::Desc => {
                        (base_qty / price).trunc_with_scale(order_symbol.base_asset_precision)
                    }
                };

                orders.push(PreOrder {
                    symbol: order_symbol.symbol.clone(),
                    symbol_order: order_symbol.symbol_order,
                    price,
                    base_qty,
                    base_precision: order_symbol.base_asset_precision,
                    quote_qty,
                    quote_precision: order_symbol.quote_precision,
                    symbol_filter: order_symbol.symbol_filter.clone(),
                });

                // If first symbol and base qty does not match the max order qty, where max order
                // qty for 2nd and 3rd symbol is previous symbol quote qty, it is necessary to
                // recalculate the qty of previous orders.
                if i != 0 && base_qty < max_order_qty {
                    Self::recalculate_orders_qty(&mut orders, i);
                }
            }

            // Compare first chain order qty and first chain item qty limit.
            // If it is equal, there is no point in trying to sum up the qty, so break.
            if orders[orders.len() - chain.len()].base_qty == max_order_qty {
                break;
            }

            start_depth_limit += 1;
        }

        // Round and recalculate quantities according to binance api rules.
        let mut profit_orders = vec![];
        let mut min_profit_qty = get_min_profit_qty(chain.first().unwrap());

        // Iterate over every first order in chain.
        'outer_loop: for i in (0..).take(orders.len() - 1).step_by(chain.len()) {
            let mut count = 0;
            let mut tmp_orders: Vec<ChainOrder> = vec![];

            while count < chain.len() {
                let price = orders[count]
                    .price
                    .trunc_with_scale(orders[count].symbol_filter.tick_size);

                let base_qty = if count == 0 {
                    orders[i].base_qty
                } else {
                    tmp_orders[count - 1].quote_qty
                };

                let (rounded_base_qty, rounded_quote_qty) = match orders[count].symbol_order {
                    SymbolOrder::Asc => {
                        let base_qty =
                            base_qty.trunc_with_scale(orders[count].symbol_filter.lot_size_step);

                        // If at least one order from the chain does not have enough quantity to
                        // reach the minimum, then skip the entire chain of orders.
                        if orders[count].symbol_filter.lot_size_min_qty > base_qty {
                            continue 'outer_loop;
                        }

                        (base_qty, base_qty * price)
                    }
                    SymbolOrder::Desc => {
                        let quote_qty = (base_qty / price)
                            .trunc_with_scale(orders[count].symbol_filter.lot_size_step);

                        if orders[count].symbol_filter.lot_size_min_qty > quote_qty {
                            continue 'outer_loop;
                        }

                        (base_qty, quote_qty)
                    }
                };

                tmp_orders.push(ChainOrder {
                    symbol: orders[count].symbol.clone(),
                    symbol_order: orders[count].symbol_order,
                    price,
                    base_qty: rounded_base_qty,
                    quote_qty: rounded_quote_qty,
                    base_increment: Decimal::new(1i64, orders[count].symbol_filter.lot_size_step),
                    quote_increment: Decimal::zero(), // set default because not used
                });

                count += 1;
            }

            // Check profit.
            let fee = calculate_fee(tmp_orders.first().unwrap().base_qty, fee_percent);

            // Difference between the outbound volume of the last symbol in chain and the inbound
            // volume of the first symbol in chain.
            let diff_qty =
                tmp_orders.last().unwrap().quote_qty - tmp_orders.first().unwrap().base_qty;

            if (diff_qty - fee) >= min_profit_qty {
                min_profit_qty = diff_qty - fee;
                profit_orders.extend_from_slice(&tmp_orders);
            }
        }

        // Return 3 last profit orders.
        if profit_orders.len() >= chain.len() {
            let idx = profit_orders.len().sub(chain.len());
            profit_orders[idx..].to_vec()
        } else {
            profit_orders
        }
    }

    /// Recalculate quantities of orders. First order in chain always skip,
    /// because operate with a max order quantity value.
    fn recalculate_orders_qty(orders: &mut [PreOrder], order_index: usize) {
        let orders_count = orders.len();
        let mut count = 1;

        while count <= order_index {
            let order_a_idx = orders_count - count - 1;
            let order_b_idx = orders_count - count;

            let order_a = &orders[order_a_idx];
            let order_b = &orders[order_b_idx];

            if order_a.quote_qty == order_b.base_qty {
                return;
            }

            let base_precision = match order_a.symbol_order {
                SymbolOrder::Asc => order_a.base_precision,
                SymbolOrder::Desc => order_a.quote_precision,
            };

            let base_qty = match order_a.symbol_order {
                SymbolOrder::Asc => order_b.base_qty / order_a.price,
                SymbolOrder::Desc => order_b.base_qty * order_a.price,
            };

            {
                orders[order_a_idx].quote_qty = order_b.base_qty;
                orders[order_a_idx].base_qty = base_qty.trunc_with_scale(base_precision);
            }

            count += 1;
        }
    }
}

fn find_base_asset(base_assets: &[Asset], chain_symbol: &ChainSymbol) -> Option<Asset> {
    base_assets
        .iter()
        .find(|&x| {
            if chain_symbol.order == SymbolOrder::Asc {
                x.asset == chain_symbol.symbol.base_asset
            } else {
                x.asset == chain_symbol.symbol.quote_asset
            }
        })
        .cloned()
}

fn define_symbol_filter(filters: &Vec<Filters>) -> SymbolFilter {
    let mut symbol_filter = SymbolFilter::default();
    for filter in filters {
        match filter {
            Filters::LotSize {
                min_qty,
                max_qty: _max_qty,
                step_size,
            } => {
                symbol_filter.lot_size_step = step_size.normalize().scale();
                symbol_filter.lot_size_min_qty = *min_qty;
            }
            Filters::PriceFilter {
                min_price: _min_price,
                max_price: _max_price,
                tick_size,
            } => {
                symbol_filter.tick_size = tick_size.normalize().scale();
            }
            _ => {}
        };
    }

    symbol_filter
}

fn define_precision(order_symbol: &OrderSymbol) -> u32 {
    match order_symbol.symbol_order {
        SymbolOrder::Asc => order_symbol.base_asset_precision,
        SymbolOrder::Desc => order_symbol.quote_precision,
    }
}

fn get_max_order_qty(order_symbol: &OrderSymbol) -> Decimal {
    order_symbol
        .max_order_qty
        .unwrap()
        .trunc_with_scale(define_precision(order_symbol))
}

fn get_min_profit_qty(order_symbol: &OrderSymbol) -> Decimal {
    order_symbol
        .min_profit_qty
        .unwrap()
        .trunc_with_scale(define_precision(order_symbol))
}

fn calculate_fee(qty: Decimal, fee_percent: Decimal) -> Decimal {
    let orders_count = Decimal::from_usize(3).unwrap();
    let delimiter = Decimal::from_usize(100).unwrap();
    (qty * fee_percent * orders_count) / delimiter
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use engine::enums::SymbolOrder;
    use rust_decimal::prelude::FromPrimitive;

    use super::*;

    // Case #1: all orders of the 1st depth have volumes greater than the volume limit.
    // (order - ASC/DESC/ASC)
    #[tokio::test]
    async fn test_calculate_chain_profit_1() -> anyhow::Result<()> {
        let market_depth_limit: usize = 1;
        let fee_percent: Decimal = Decimal::from_str("0.075").unwrap();

        let order_book_1 = BookTickerEvent {
            update_id: 1,
            symbol: "BTCUSDT".to_owned(),
            bid_price: Decimal::from_f64(109615.46000000).unwrap(),
            bid_qty: Decimal::from_f64(7.27795000).unwrap(),
            ask_price: Decimal::from_f64(109615.47000000).unwrap(),
            ask_qty: Decimal::from_f64(2.22969000).unwrap(),
        };

        let order_book_2 = BookTickerEvent {
            update_id: 1,
            symbol: "ETHUSDT".to_owned(),
            bid_price: Decimal::from_f64(2585.70000000).unwrap(),
            bid_qty: Decimal::from_f64(14.64600000).unwrap(),
            ask_price: Decimal::from_f64(2585.71000000).unwrap(),
            ask_qty: Decimal::from_f64(19.28810000).unwrap(),
        };

        let order_book_3 = BookTickerEvent {
            update_id: 1,
            symbol: "ETHBTC".to_owned(),
            bid_price: Decimal::from_f64(0.02858000).unwrap(),
            bid_qty: Decimal::from_f64(105.74550000).unwrap(),
            ask_price: Decimal::from_f64(0.02359000).unwrap(),
            ask_qty: Decimal::from_f64(25.63400000).unwrap(),
        };

        let order_symbols = vec![
            OrderSymbol {
                symbol: "BTCUSDT".to_owned(),
                base_asset_precision: 8,
                quote_precision: 8,
                symbol_order: SymbolOrder::Asc,
                min_profit_qty: Decimal::from_f64(0.000030),
                max_order_qty: Decimal::from_f64(0.00030),
                order_book: &order_book_1,
                symbol_filter: SymbolFilter {
                    lot_size_step: 5,
                    tick_size: 2,
                    lot_size_min_qty: Decimal::from_f64(0.00001000).unwrap(),
                },
            },
            OrderSymbol {
                symbol: "ETHUSDT".to_owned(),
                base_asset_precision: 8,
                quote_precision: 8,
                symbol_order: SymbolOrder::Desc,
                min_profit_qty: None,
                max_order_qty: None,
                order_book: &order_book_2,
                symbol_filter: SymbolFilter {
                    lot_size_step: 4,
                    tick_size: 2,
                    lot_size_min_qty: Decimal::from_f64(0.00010000).unwrap(),
                },
            },
            OrderSymbol {
                symbol: "ETHBTC".to_owned(),
                base_asset_precision: 8,
                quote_precision: 8,
                symbol_order: SymbolOrder::Asc,
                min_profit_qty: None,
                max_order_qty: None,
                order_book: &order_book_3,
                symbol_filter: SymbolFilter {
                    lot_size_step: 4,
                    tick_size: 5,
                    lot_size_min_qty: Decimal::from_f64(0.00010000).unwrap(),
                },
            },
        ];

        let orders =
            OrderBuilder::calculate_chain_profit(&order_symbols, market_depth_limit, fee_percent);

        assert_eq!(orders.len(), 3);

        assert_eq!(orders[0].symbol, "BTCUSDT");
        assert_eq!(orders[0].symbol_order, SymbolOrder::Asc);
        assert_eq!(orders[0].price.to_string(), "109615.46");
        assert_eq!(orders[0].base_qty.to_string(), "0.00030");
        assert_eq!(orders[0].quote_qty.to_string(), "32.8846380");

        assert_eq!(orders[1].symbol, "ETHUSDT");
        assert_eq!(orders[1].symbol_order, SymbolOrder::Desc);
        assert_eq!(orders[1].price.to_string(), "2585.71");
        assert_eq!(orders[1].base_qty.to_string(), "32.8846380");
        assert_eq!(orders[1].quote_qty.to_string(), "0.0127");

        assert_eq!(orders[2].symbol, "ETHBTC");
        assert_eq!(orders[2].symbol_order, SymbolOrder::Asc);
        assert_eq!(orders[2].price.to_string(), "0.02858");
        assert_eq!(orders[2].base_qty.to_string(), "0.0127");
        assert_eq!(orders[2].quote_qty.to_string(), "0.000362966");

        Ok(())
    }

    // Case #2: 1st pair of 1st depth does not have enough volume to reach the volume limit.
    // (order - ASC/DESC/ASC)
    #[tokio::test]
    async fn test_calculate_chain_profit_2() -> anyhow::Result<()> {
        let market_depth_limit: usize = 1;
        let fee_percent: Decimal = Decimal::from_str("0.075").unwrap();

        let order_book_1 = BookTickerEvent {
            update_id: 1,
            symbol: "BTCUSDT".to_owned(),
            bid_price: Decimal::from_f64(109615.46000000).unwrap(),
            bid_qty: Decimal::from_f64(0.00020000).unwrap(), // <---- here,
            ask_price: Decimal::from_f64(109615.47000000).unwrap(),
            ask_qty: Decimal::from_f64(2.22969000).unwrap(),
        };

        let order_book_2 = BookTickerEvent {
            update_id: 1,
            symbol: "ETHUSDT".to_owned(),
            bid_price: Decimal::from_f64(2585.70000000).unwrap(),
            bid_qty: Decimal::from_f64(14.64600000).unwrap(),
            ask_price: Decimal::from_f64(2585.71000000).unwrap(),
            ask_qty: Decimal::from_f64(19.28810000).unwrap(),
        };

        let order_book_3 = BookTickerEvent {
            update_id: 1,
            symbol: "ETHBTC".to_owned(),
            bid_price: Decimal::from_f64(0.02858000).unwrap(),
            bid_qty: Decimal::from_f64(105.74550000).unwrap(),
            ask_price: Decimal::from_f64(0.02359000).unwrap(),
            ask_qty: Decimal::from_f64(25.63400000).unwrap(),
        };

        let order_symbols = vec![
            OrderSymbol {
                symbol: "BTCUSDT".to_owned(),
                base_asset_precision: 8,
                quote_precision: 8,
                symbol_order: SymbolOrder::Asc,
                min_profit_qty: Decimal::from_f64(0.0),
                max_order_qty: Decimal::from_f64(0.00030),
                order_book: &order_book_1,
                symbol_filter: SymbolFilter {
                    lot_size_step: 5,
                    tick_size: 2,
                    lot_size_min_qty: Decimal::from_f64(0.00001000).unwrap(),
                },
            },
            OrderSymbol {
                symbol: "ETHUSDT".to_owned(),
                base_asset_precision: 8,
                quote_precision: 8,
                symbol_order: SymbolOrder::Desc,
                min_profit_qty: None,
                max_order_qty: None,
                order_book: &order_book_2,
                symbol_filter: SymbolFilter {
                    lot_size_step: 4,
                    tick_size: 2,
                    lot_size_min_qty: Decimal::from_f64(0.00010000).unwrap(),
                },
            },
            OrderSymbol {
                symbol: "ETHBTC".to_owned(),
                base_asset_precision: 8,
                quote_precision: 8,
                symbol_order: SymbolOrder::Asc,
                min_profit_qty: None,
                max_order_qty: None,
                order_book: &order_book_3,
                symbol_filter: SymbolFilter {
                    lot_size_step: 4,
                    tick_size: 5,
                    lot_size_min_qty: Decimal::from_f64(0.00010000).unwrap(),
                },
            },
        ];

        let orders =
            OrderBuilder::calculate_chain_profit(&order_symbols, market_depth_limit, fee_percent);

        assert_eq!(orders.len(), 3);

        assert_eq!(orders[0].symbol, "BTCUSDT");
        assert_eq!(orders[0].symbol_order, SymbolOrder::Asc);
        assert_eq!(orders[0].price.to_string(), "109615.46");
        assert_eq!(orders[0].base_qty.to_string(), "0.00020");
        assert_eq!(orders[0].quote_qty.to_string(), "21.9230920");

        assert_eq!(orders[1].symbol, "ETHUSDT");
        assert_eq!(orders[1].symbol_order, SymbolOrder::Desc);
        assert_eq!(orders[1].price.to_string(), "2585.71");
        assert_eq!(orders[1].base_qty.to_string(), "21.9230920");
        assert_eq!(orders[1].quote_qty.to_string(), "0.0084");

        assert_eq!(orders[2].symbol, "ETHBTC");
        assert_eq!(orders[2].symbol_order, SymbolOrder::Asc);
        assert_eq!(orders[2].price.to_string(), "0.02858");
        assert_eq!(orders[2].base_qty.to_string(), "0.0084");
        assert_eq!(orders[2].quote_qty.to_string(), "0.000240072");

        Ok(())
    }

    // Case #3: the 2nd pair of the 1st depth does not have enough volume to reach the volume
    // (order - ASC/DESC/ASC)
    #[tokio::test]
    async fn test_calculate_chain_profit_3() -> anyhow::Result<()> {
        let market_depth_limit: usize = 1;
        let fee_percent: Decimal = Decimal::from_str("0.075").unwrap();

        let order_book_1 = BookTickerEvent {
            update_id: 1,
            symbol: "BTCUSDT".to_owned(),
            bid_price: Decimal::from_f64(109615.46000000).unwrap(),
            bid_qty: Decimal::from_f64(0.20000000).unwrap(),
            ask_price: Decimal::from_f64(109615.47000000).unwrap(),
            ask_qty: Decimal::from_f64(2.22969000).unwrap(),
        };

        let order_book_2 = BookTickerEvent {
            update_id: 1,
            symbol: "ETHUSDT".to_owned(),
            bid_price: Decimal::from_f64(1585.70000000).unwrap(),
            bid_qty: Decimal::from_f64(19.28810000).unwrap(),
            ask_price: Decimal::from_f64(1585.71000000).unwrap(),
            ask_qty: Decimal::from_f64(0.0033).unwrap(), // <---- here
        };

        let order_book_3 = BookTickerEvent {
            update_id: 1,
            symbol: "ETHBTC".to_owned(),
            bid_price: Decimal::from_f64(0.02858000).unwrap(),
            bid_qty: Decimal::from_f64(105.74550000).unwrap(),
            ask_price: Decimal::from_f64(0.02359000).unwrap(),
            ask_qty: Decimal::from_f64(25.63400000).unwrap(),
        };

        let order_symbols = vec![
            OrderSymbol {
                symbol: "BTCUSDT".to_owned(),
                base_asset_precision: 8,
                quote_precision: 8,
                symbol_order: SymbolOrder::Asc,
                min_profit_qty: Decimal::from_f64(0.000030),
                max_order_qty: Decimal::from_f64(0.00030),
                order_book: &order_book_1,
                symbol_filter: SymbolFilter {
                    lot_size_step: 5,
                    tick_size: 2,
                    lot_size_min_qty: Decimal::from_f64(0.00001000).unwrap(),
                },
            },
            OrderSymbol {
                symbol: "ETHUSDT".to_owned(),
                base_asset_precision: 8,
                quote_precision: 8,
                symbol_order: SymbolOrder::Desc,
                min_profit_qty: None,
                max_order_qty: None,
                order_book: &order_book_2,
                symbol_filter: SymbolFilter {
                    lot_size_step: 4,
                    tick_size: 2,
                    lot_size_min_qty: Decimal::from_f64(0.00010000).unwrap(),
                },
            },
            OrderSymbol {
                symbol: "ETHBTC".to_owned(),
                base_asset_precision: 8,
                quote_precision: 8,
                symbol_order: SymbolOrder::Asc,
                min_profit_qty: None,
                max_order_qty: None,
                order_book: &order_book_3,
                symbol_filter: SymbolFilter {
                    lot_size_step: 4,
                    tick_size: 5,
                    lot_size_min_qty: Decimal::from_f64(0.00010000).unwrap(),
                },
            },
        ];

        let orders =
            OrderBuilder::calculate_chain_profit(&order_symbols, market_depth_limit, fee_percent);

        assert_eq!(orders.len(), 3);

        assert_eq!(orders[0].symbol, "BTCUSDT");
        assert_eq!(orders[0].symbol_order, SymbolOrder::Asc);
        assert_eq!(orders[0].price.to_string(), "109615.46");
        assert_eq!(orders[0].base_qty.to_string(), "0.00004");
        assert_eq!(orders[0].quote_qty.to_string(), "4.3846184");

        assert_eq!(orders[1].symbol, "ETHUSDT");
        assert_eq!(orders[1].symbol_order, SymbolOrder::Desc);
        assert_eq!(orders[1].price.to_string(), "1585.71");
        assert_eq!(orders[1].base_qty.to_string(), "4.3846184");
        assert_eq!(orders[1].quote_qty.to_string(), "0.0027");

        assert_eq!(orders[2].symbol, "ETHBTC");
        assert_eq!(orders[2].symbol_order, SymbolOrder::Asc);
        assert_eq!(orders[2].price.to_string(), "0.02858");
        assert_eq!(orders[2].base_qty.to_string(), "0.0027");
        assert_eq!(orders[2].quote_qty.to_string(), "0.000077166");

        Ok(())
    }

    // Case #3: the 3rd pair of the 1st depth does not have enough volume to reach the volume
    // (order - ASC/DESC/ASC)
    #[tokio::test]
    async fn test_calculate_chain_profit_4() -> anyhow::Result<()> {
        let market_depth_limit: usize = 1;
        let fee_percent: Decimal = Decimal::from_str("0.075").unwrap();

        let order_book_1 = BookTickerEvent {
            update_id: 1,
            symbol: "BTCUSDT".to_owned(),
            bid_price: Decimal::from_f64(109615.46000000).unwrap(),
            bid_qty: Decimal::from_f64(0.20000000).unwrap(),
            ask_price: Decimal::from_f64(109615.47000000).unwrap(),
            ask_qty: Decimal::from_f64(2.22969000).unwrap(),
        };

        let order_book_2 = BookTickerEvent {
            update_id: 1,
            symbol: "ETHUSDT".to_owned(),
            bid_price: Decimal::from_f64(2585.70000000).unwrap(),
            bid_qty: Decimal::from_f64(19.28810000).unwrap(),
            ask_price: Decimal::from_f64(2585.71000000).unwrap(),
            ask_qty: Decimal::from_f64(0.9).unwrap(),
        };

        let order_book_3 = BookTickerEvent {
            update_id: 1,
            symbol: "ETHBTC".to_owned(),
            bid_price: Decimal::from_f64(0.02858000).unwrap(),
            bid_qty: Decimal::from_f64(0.01).unwrap(), // <---- here,
            ask_price: Decimal::from_f64(0.02359000).unwrap(),
            ask_qty: Decimal::from_f64(25.63400000).unwrap(),
        };

        let order_symbols = vec![
            OrderSymbol {
                symbol: "BTCUSDT".to_owned(),
                base_asset_precision: 8,
                quote_precision: 8,
                symbol_order: SymbolOrder::Asc,
                min_profit_qty: Decimal::from_f64(0.000030),
                max_order_qty: Decimal::from_f64(0.00030),
                order_book: &order_book_1,
                symbol_filter: SymbolFilter {
                    lot_size_step: 5,
                    tick_size: 2,
                    lot_size_min_qty: Decimal::from_f64(0.00001000).unwrap(),
                },
            },
            OrderSymbol {
                symbol: "ETHUSDT".to_owned(),
                base_asset_precision: 8,
                quote_precision: 8,
                symbol_order: SymbolOrder::Desc,
                min_profit_qty: None,
                max_order_qty: None,
                order_book: &order_book_2,
                symbol_filter: SymbolFilter {
                    lot_size_step: 4,
                    tick_size: 2,
                    lot_size_min_qty: Decimal::from_f64(0.00010000).unwrap(),
                },
            },
            OrderSymbol {
                symbol: "ETHBTC".to_owned(),
                base_asset_precision: 8,
                quote_precision: 8,
                symbol_order: SymbolOrder::Asc,
                min_profit_qty: None,
                max_order_qty: None,
                order_book: &order_book_3,
                symbol_filter: SymbolFilter {
                    lot_size_step: 4,
                    tick_size: 5,
                    lot_size_min_qty: Decimal::from_f64(0.00010000).unwrap(),
                },
            },
        ];

        let orders =
            OrderBuilder::calculate_chain_profit(&order_symbols, market_depth_limit, fee_percent);

        assert_eq!(orders.len(), 3);

        assert_eq!(orders[0].symbol, "BTCUSDT");
        assert_eq!(orders[0].symbol_order, SymbolOrder::Asc);
        assert_eq!(orders[0].price.to_string(), "109615.46");
        assert_eq!(orders[0].base_qty.to_string(), "0.00023");
        assert_eq!(orders[0].quote_qty.to_string(), "25.2115558");

        assert_eq!(orders[1].symbol, "ETHUSDT");
        assert_eq!(orders[1].symbol_order, SymbolOrder::Desc);
        assert_eq!(orders[1].price.to_string(), "2585.71");
        assert_eq!(orders[1].base_qty.to_string(), "25.2115558");
        assert_eq!(orders[1].quote_qty.to_string(), "0.0097");

        assert_eq!(orders[2].symbol, "ETHBTC");
        assert_eq!(orders[2].symbol_order, SymbolOrder::Asc);
        assert_eq!(orders[2].price.to_string(), "0.02858");
        assert_eq!(orders[2].base_qty.to_string(), "0.0097");
        assert_eq!(orders[2].quote_qty.to_string(), "0.000277226");

        Ok(())
    }

    // Case: skipped, does not pass the minimum quantity.
    #[tokio::test]
    async fn test_calculate_chain_profit_5() -> anyhow::Result<()> {
        let market_depth_limit: usize = 1;
        let fee_percent: Decimal = Decimal::from_str("0.075").unwrap();

        let order_book_1 = BookTickerEvent {
            update_id: 1,
            symbol: "ETHBTC".to_owned(),
            bid_price: Decimal::from_f64(0.03615000).unwrap(),
            bid_qty: Decimal::from_f64(0.20000000).unwrap(),
            ask_price: Decimal::from_f64(0.03216000).unwrap(),
            ask_qty: Decimal::from_f64(2.22969000).unwrap(),
        };

        let order_book_2 = BookTickerEvent {
            update_id: 1,
            symbol: "WBTCBTC".to_owned(),
            bid_price: Decimal::from_f64(0.99920000).unwrap(),
            bid_qty: Decimal::from_f64(19.28810000).unwrap(),
            ask_price: Decimal::from_f64(0.99930000).unwrap(),
            ask_qty: Decimal::from_f64(0.9).unwrap(),
        };

        let order_book_3 = BookTickerEvent {
            update_id: 1,
            symbol: "WBTCETH".to_owned(),
            bid_price: Decimal::from_f64(31.07000000).unwrap(),
            bid_qty: Decimal::from_f64(1.5).unwrap(), // <---- here
            ask_price: Decimal::from_f64(31.08000000).unwrap(),
            ask_qty: Decimal::from_f64(25.63400000).unwrap(),
        };

        let order_symbols = vec![
            OrderSymbol {
                symbol: "ETHBTC".to_owned(),
                base_asset_precision: 8,
                quote_precision: 8,
                symbol_order: SymbolOrder::Asc,
                min_profit_qty: Decimal::from_f64(0.0),
                max_order_qty: Decimal::from_f64(0.0079),
                order_book: &order_book_1,
                symbol_filter: SymbolFilter {
                    lot_size_step: 4,
                    tick_size: 5,
                    lot_size_min_qty: Decimal::from_f64(0.00010000).unwrap(),
                },
            },
            OrderSymbol {
                symbol: "WBTCBTC".to_owned(),
                base_asset_precision: 8,
                quote_precision: 8,
                symbol_order: SymbolOrder::Desc,
                min_profit_qty: None,
                max_order_qty: None,
                order_book: &order_book_2,
                symbol_filter: SymbolFilter {
                    lot_size_step: 5,
                    tick_size: 4,
                    lot_size_min_qty: Decimal::from_f64(0.00010000).unwrap(),
                },
            },
            OrderSymbol {
                symbol: "WBTCETH".to_owned(),
                base_asset_precision: 8,
                quote_precision: 8,
                symbol_order: SymbolOrder::Asc,
                min_profit_qty: None,
                max_order_qty: None,
                order_book: &order_book_3,
                symbol_filter: SymbolFilter {
                    lot_size_step: 5,
                    tick_size: 2,
                    lot_size_min_qty: Decimal::from_f64(0.00100000).unwrap(),
                },
            },
        ];

        let orders =
            OrderBuilder::calculate_chain_profit(&order_symbols, market_depth_limit, fee_percent);
        assert_eq!(orders.len(), 0);

        Ok(())
    }

    #[tokio::test]
    async fn test_calculate_chain_profit_6() -> anyhow::Result<()> {
        let market_depth_limit: usize = 1;
        let fee_percent: Decimal = Decimal::from_str("0.075").unwrap();

        let order_book_1 = BookTickerEvent {
            update_id: 1,
            symbol: "ETHBTC".to_owned(),
            bid_price: Decimal::from_f64(0.03402000).unwrap(),
            bid_qty: Decimal::from_f64(23.09700000).unwrap(),
            ask_price: Decimal::from_f64(0.03203000).unwrap(),
            ask_qty: Decimal::from_f64(23.09700000).unwrap(),
        };

        let order_book_2 = BookTickerEvent {
            update_id: 1,
            symbol: "WBTCBTC".to_owned(),
            bid_price: Decimal::from_f64(0.00007820).unwrap(),
            bid_qty: Decimal::from_f64(1.62000000).unwrap(),
            ask_price: Decimal::from_f64(0.00007810).unwrap(),
            ask_qty: Decimal::from_f64(1.62000000).unwrap(),
        };

        let order_book_3 = BookTickerEvent {
            update_id: 1,
            symbol: "WBTCETH".to_owned(),
            bid_price: Decimal::from_f64(0.00243200).unwrap(),
            bid_qty: Decimal::from_f64(0.54000000).unwrap(), // <---- here
            ask_price: Decimal::from_f64(0.00243300).unwrap(),
            ask_qty: Decimal::from_f64(0.54000000).unwrap(),
        };
        let order_symbols = vec![
            OrderSymbol {
                symbol: "ETHBTC".to_owned(),
                base_asset_precision: 8,
                quote_precision: 8,
                symbol_order: SymbolOrder::Asc,
                min_profit_qty: Decimal::from_f64(0.0),
                max_order_qty: Decimal::from_f64(0.0079),
                order_book: &order_book_1,
                symbol_filter: SymbolFilter {
                    lot_size_step: 4,
                    tick_size: 5,
                    lot_size_min_qty: Decimal::from_f64(0.00010000).unwrap(),
                },
            },
            OrderSymbol {
                symbol: "SSVBTC".to_owned(),
                base_asset_precision: 8,
                quote_precision: 8,
                symbol_order: SymbolOrder::Desc,
                min_profit_qty: None,
                max_order_qty: None,
                order_book: &order_book_2,
                symbol_filter: SymbolFilter {
                    lot_size_step: 2,
                    tick_size: 7,
                    lot_size_min_qty: Decimal::from_f64(0.00010000).unwrap(),
                },
            },
            OrderSymbol {
                symbol: "SSVETH".to_owned(),
                base_asset_precision: 8,
                quote_precision: 8,
                symbol_order: SymbolOrder::Asc,
                min_profit_qty: None,
                max_order_qty: None,
                order_book: &order_book_3,
                symbol_filter: SymbolFilter {
                    lot_size_step: 2,
                    tick_size: 6,
                    lot_size_min_qty: Decimal::from_f64(0.00100000).unwrap(),
                },
            },
        ];

        let orders =
            OrderBuilder::calculate_chain_profit(&order_symbols, market_depth_limit, fee_percent);
        assert_eq!(orders.len(), 3);

        assert_eq!(orders[0].symbol, "ETHBTC");
        assert_eq!(orders[0].symbol_order, SymbolOrder::Asc);
        assert_eq!(orders[0].price.to_string(), "0.03402");
        assert_eq!(orders[0].base_qty.to_string(), "0.0012");
        assert_eq!(orders[0].quote_qty.to_string(), "0.000040824");

        assert_eq!(orders[1].symbol, "SSVBTC");
        assert_eq!(orders[1].symbol_order, SymbolOrder::Desc);
        assert_eq!(orders[1].price.to_string(), "0.0000781");
        assert_eq!(orders[1].base_qty.to_string(), "0.000040824");
        assert_eq!(orders[1].quote_qty.to_string(), "0.52");

        assert_eq!(orders[2].symbol, "SSVETH");
        assert_eq!(orders[2].symbol_order, SymbolOrder::Asc);
        assert_eq!(orders[2].price.to_string(), "0.002432");
        assert_eq!(orders[2].base_qty.to_string(), "0.52");
        assert_eq!(orders[2].quote_qty.to_string(), "0.00126464");

        Ok(())
    }
}
