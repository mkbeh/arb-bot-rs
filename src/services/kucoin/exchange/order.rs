use std::{ops::Sub, sync::Arc};

use itertools::Itertools;
use rust_decimal::{
    Decimal,
    prelude::{FromPrimitive, Zero},
};
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;
use tracing::error;
use uuid::Uuid;

use crate::{
    config::Asset,
    libs::misc,
    services::{
        Chain, ORDERS_CHANNEL, Order, OrderBookUnit,
        enums::SymbolOrder,
        kucoin::{
            broadcast::TICKER_BROADCAST,
            exchange::{chain, chain::ChainSymbol},
            storage::{BookTickerEvent, BookTickerEventChanges, BookTickerStore},
        },
        metrics::METRICS,
    },
};

#[derive(Clone, Debug)]
pub struct OrderSymbol<'a> {
    pub symbol: String,
    pub symbol_order: SymbolOrder,
    pub order_book: &'a BookTickerEvent,
    pub base_min_size: Decimal,
    pub quote_min_size: Decimal,
    pub base_max_size: Decimal,
    pub quote_max_size: Decimal,
    pub base_increment: Decimal,
    pub quote_increment: Decimal,
    pub price_increment: Decimal,
    pub min_profit_qty: Option<Decimal>,
    pub max_order_qty: Option<Decimal>,
}

#[derive(Clone, Debug)]
pub struct PreOrder {
    symbol: String,
    symbol_order: SymbolOrder,
    price: Decimal,
    base_qty: Decimal,
    quote_qty: Decimal,
    base_min_size: Decimal,
    _quote_min_size: Decimal,
    _base_max_size: Decimal,
    _quote_max_size: Decimal,
    base_increment: Decimal,
    quote_increment: Decimal,
    price_increment: Decimal,
}

pub struct OrderBuilder {
    market_depth_limit: usize,
    fee_percent: Decimal,
}

impl OrderBuilder {
    pub fn new(market_depth_limit: usize, fee_percent: Decimal) -> Self {
        Self {
            market_depth_limit,
            fee_percent,
        }
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

                    let mut bid_storage = BookTickerStore::new();
                    let mut ask_storage = BookTickerStore::new();
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
                                this.handle_ticker_event(&mut bid_storage, &mut ask_storage, &chain, msg, &mut last_prices, &base_assets);
                            },

                            _ = rx2.changed() => {
                                let msg = rx2.borrow().clone();
                                this.handle_ticker_event(&mut bid_storage, &mut ask_storage, &chain, msg, &mut last_prices, &base_assets);
                            },

                            _ = rx3.changed() => {
                                let msg = rx3.borrow().clone();
                                this.handle_ticker_event(&mut bid_storage, &mut ask_storage, &chain, msg, &mut last_prices, &base_assets);
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

    pub fn handle_ticker_event(
        &self,
        bid_storage: &mut BookTickerStore,
        ask_storage: &mut BookTickerStore,
        chain: &[ChainSymbol; 3],
        msg: BookTickerEventChanges,
        last_prices: &mut Vec<Decimal>,
        base_assets: &[Asset],
    ) {
        if !bid_storage.update_if_valid(msg.bid) && !ask_storage.update_if_valid(msg.ask) {
            return;
        }

        // Early return if not all data is available
        let messages: Vec<BookTickerEvent> = chain
            .iter()
            .filter_map(|symbol| match symbol.order {
                SymbolOrder::Asc => bid_storage.get(symbol.symbol.symbol.as_str()).cloned(),
                SymbolOrder::Desc => ask_storage.get(symbol.symbol.symbol.as_str()).cloned(),
            })
            .collect();

        if messages.len() != chain.len() {
            return;
        }

        // Calculate prices
        let prices = messages.iter().map(|m| m.price).collect::<Vec<Decimal>>();

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

    /// Build orders info and calculate profit.
    pub fn process_chain(
        base_assets: &[Asset],
        chain: &[ChainSymbol; 3],
        order_book: &[BookTickerEvent],
        market_depth_limit: usize,
        fee_percent: Decimal,
    ) -> anyhow::Result<()> {
        let mut order_symbols = vec![];

        for (i, chain_symbol) in chain.iter().enumerate() {
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
            order_symbols.push(OrderSymbol {
                symbol: symbol.symbol.clone(),
                symbol_order: chain_symbol.order,
                order_book: &order_book[i],
                base_min_size: symbol.base_min_size,
                quote_min_size: symbol.quote_min_size,
                base_max_size: symbol.base_max_size,
                quote_max_size: symbol.quote_max_size,
                base_increment: symbol.base_increment,
                quote_increment: symbol.quote_increment,
                price_increment: symbol.price_increment,
                min_profit_qty,
                max_order_qty,
            });
        }

        let orders = Self::calculate_chain_profit(&order_symbols, market_depth_limit, fee_percent);
        METRICS.add_processed_chain(&chain::extract_chain_symbols(chain));

        if orders.is_empty() {
            return Ok(());
        }

        let orders_chain = Chain {
            ts: misc::time::get_current_timestamp().as_millis(),
            chain_id: Uuid::new_v4(),
            fee_percent,
            orders,
        };

        if let Err(e) = ORDERS_CHANNEL.tx.send(orders_chain) {
            error!(error = ?e, "Failed to send chain to channel");
        }

        Ok(())
    }

    pub fn calculate_chain_profit(
        chain: &[OrderSymbol],
        market_depth_limit: usize,
        fee_percent: Decimal,
    ) -> Vec<Order> {
        let mut orders: Vec<PreOrder> = vec![];
        let mut start_depth_limit = 0;

        // Extract max order qty from first symbol in the chain.
        let max_order_qty = get_max_order_qty(chain.first().unwrap());

        while start_depth_limit < market_depth_limit {
            for (i, order_symbol) in chain.iter().enumerate() {
                // Define list of orders according to the order of assets in symbol.
                let order_units: Vec<OrderBookUnit> = vec![OrderBookUnit {
                    price: order_symbol.order_book.price,
                    qty: order_symbol.order_book.qty,
                }];

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
                        SymbolOrder::Desc => order_unit.qty * order_unit.price,
                    };

                    price = order_unit.price;
                    base_qty += qty;

                    if base_qty >= max_order_qty {
                        base_qty = max_order_qty;
                        break;
                    }
                }

                let quote_qty = match order_symbol.symbol_order {
                    SymbolOrder::Asc => base_qty * price,
                    SymbolOrder::Desc => base_qty / price,
                };

                orders.push(PreOrder {
                    symbol: order_symbol.symbol.clone(),
                    symbol_order: order_symbol.symbol_order,
                    base_min_size: order_symbol.base_min_size,
                    _quote_min_size: order_symbol.quote_min_size,
                    _base_max_size: order_symbol.base_max_size,
                    _quote_max_size: order_symbol.quote_max_size,
                    base_increment: order_symbol.base_increment,
                    quote_increment: order_symbol.quote_increment,
                    price_increment: order_symbol.price_increment,
                    price,
                    base_qty,
                    quote_qty,
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
            let mut tmp_orders: Vec<Order> = vec![];

            while count < chain.len() {
                let order = &orders[count];
                let price_scale = order.price_increment.scale();
                let base_scale = order.base_increment.scale();
                let quote_scale = order.quote_increment.scale();

                let price = order.price.trunc_with_scale(price_scale);
                let base_qty = if count == 0 {
                    orders[i].base_qty
                } else {
                    tmp_orders[count - 1].quote_qty
                };

                let (rounded_base_qty, rounded_quote_qty) = match order.symbol_order {
                    SymbolOrder::Asc => {
                        let base_qty = base_qty.trunc_with_scale(base_scale);
                        let quote_qty = (base_qty * price).trunc_with_scale(quote_scale);

                        // If at least one order from the chain does not have enough quantity to
                        // reach the minimum, then skip the entire chain of orders.
                        if order.base_min_size > base_qty {
                            continue 'outer_loop;
                        }

                        (base_qty, quote_qty)
                    }
                    SymbolOrder::Desc => {
                        let base_qty = base_qty.trunc_with_scale(quote_scale);
                        let quote_qty = (base_qty / price).trunc_with_scale(base_scale);

                        if order.base_min_size > quote_qty {
                            continue 'outer_loop;
                        }

                        (base_qty, quote_qty)
                    }
                };

                tmp_orders.push(Order {
                    symbol: order.symbol.clone(),
                    symbol_order: order.symbol_order,
                    base_qty: rounded_base_qty,
                    quote_qty: rounded_quote_qty,
                    base_increment: order.base_increment,
                    quote_increment: order.quote_increment,
                    price,
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

            let base_qty = match order_a.symbol_order {
                SymbolOrder::Asc => order_b.base_qty / order_a.price,
                SymbolOrder::Desc => order_b.base_qty * order_a.price,
            };

            {
                orders[order_a_idx].quote_qty = order_b.base_qty;
                orders[order_a_idx].base_qty = base_qty;
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
                x.asset == chain_symbol.symbol.base_currency
            } else {
                x.asset == chain_symbol.symbol.quote_currency
            }
        })
        .cloned()
}

fn define_precision(order_symbol: &OrderSymbol) -> u32 {
    match order_symbol.symbol_order {
        SymbolOrder::Asc => order_symbol.base_increment.scale(),
        SymbolOrder::Desc => order_symbol.quote_increment.scale(),
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

    use rust_decimal::prelude::FromPrimitive;

    use super::*;
    use crate::services::enums::SymbolOrder;

    // Case #1: all orders of the 1st depth have volumes greater than the volume limit.
    // (order - ASC/DESC/ASC)
    #[tokio::test]
    async fn test_calculate_chain_profit_1() -> anyhow::Result<()> {
        let market_depth_limit: usize = 1;
        let fee_percent: Decimal = Decimal::from_str("0.075").unwrap();

        let order_book_1 = BookTickerEvent {
            sequence_id: 0,
            symbol: "BTCUSDT".to_string(),
            price: Decimal::from_f64(109615.46000000).unwrap(),
            qty: Decimal::from_f64(7.27795000).unwrap(),
        };

        let order_book_2 = BookTickerEvent {
            sequence_id: 0,
            symbol: "ETHUSDT".to_string(),
            price: Decimal::from_f64(2585.70000000).unwrap(),
            qty: Decimal::from_f64(14.64600000).unwrap(),
        };

        let order_book_3 = BookTickerEvent {
            sequence_id: 0,
            symbol: "ETHBTC".to_string(),
            price: Decimal::from_f64(0.02858000).unwrap(),
            qty: Decimal::from_f64(105.74550000).unwrap(),
        };

        let order_symbols = vec![
            OrderSymbol {
                symbol: "BTCUSDT".to_string(),
                symbol_order: SymbolOrder::Asc,
                min_profit_qty: Decimal::from_f64(0.000030),
                max_order_qty: Decimal::from_f64(0.00030),
                order_book: &order_book_1,
                base_min_size: Default::default(),
                quote_min_size: Default::default(),
                base_max_size: Default::default(),
                quote_max_size: Default::default(),
                base_increment: Decimal::from_f64(0.00000001).unwrap(),
                quote_increment: Decimal::from_f64(0.00000001).unwrap(),
                price_increment: Decimal::from_f64(0.00000001).unwrap(),
            },
            OrderSymbol {
                symbol: "ETHUSDT".to_string(),
                symbol_order: SymbolOrder::Desc,
                min_profit_qty: None,
                max_order_qty: None,
                order_book: &order_book_2,
                base_min_size: Default::default(),
                quote_min_size: Default::default(),
                base_max_size: Default::default(),
                quote_max_size: Default::default(),
                base_increment: Decimal::from_f64(0.00000001).unwrap(),
                quote_increment: Decimal::from_f64(0.00000001).unwrap(),
                price_increment: Decimal::from_f64(0.00000001).unwrap(),
            },
            OrderSymbol {
                symbol: "ETHBTC".to_string(),
                symbol_order: SymbolOrder::Asc,
                min_profit_qty: None,
                max_order_qty: None,
                order_book: &order_book_3,
                base_min_size: Default::default(),
                quote_min_size: Default::default(),
                base_max_size: Default::default(),
                quote_max_size: Default::default(),
                base_increment: Decimal::from_f64(0.00000001).unwrap(),
                quote_increment: Decimal::from_f64(0.00000001).unwrap(),
                price_increment: Decimal::from_f64(0.00000001).unwrap(),
            },
        ];

        let orders =
            OrderBuilder::calculate_chain_profit(&order_symbols, market_depth_limit, fee_percent);

        assert_eq!(orders.len(), 3);

        assert_eq!(orders[0].symbol, "BTCUSDT");
        assert_eq!(orders[0].symbol_order, SymbolOrder::Asc);
        assert_eq!(orders[0].price.to_string(), "109615.46000000");
        assert_eq!(orders[0].base_qty.to_string(), "0.00030000");
        assert_eq!(orders[0].quote_qty.to_string(), "32.88463800");

        assert_eq!(orders[1].symbol, "ETHUSDT");
        assert_eq!(orders[1].symbol_order, SymbolOrder::Desc);
        assert_eq!(orders[1].price.to_string(), "2585.70000000");
        assert_eq!(orders[1].base_qty.to_string(), "32.88463800");
        assert_eq!(orders[1].quote_qty.to_string(), "0.01271788");

        assert_eq!(orders[2].symbol, "ETHBTC");
        assert_eq!(orders[2].symbol_order, SymbolOrder::Asc);
        assert_eq!(orders[2].price.to_string(), "0.02858000");
        assert_eq!(orders[2].base_qty.to_string(), "0.01271788");
        assert_eq!(orders[2].quote_qty.to_string(), "0.00036347");

        Ok(())
    }

    // Case #2: 1st pair of 1st depth does not have enough volume to reach the volume limit.
    // (order - ASC/DESC/ASC)
    #[tokio::test]
    async fn test_calculate_chain_profit_2() -> anyhow::Result<()> {
        let market_depth_limit: usize = 1;
        let fee_percent: Decimal = Decimal::from_str("0.075").unwrap();

        let order_book_1 = BookTickerEvent {
            sequence_id: 0,
            symbol: "BTCUSDT".to_string(),
            price: Decimal::from_f64(109615.46000000).unwrap(),
            qty: Decimal::from_f64(0.00020000).unwrap(),
        };

        let order_book_2 = BookTickerEvent {
            sequence_id: 0,
            symbol: "ETHUSDT".to_string(),
            price: Decimal::from_f64(2585.70000000).unwrap(),
            qty: Decimal::from_f64(14.64600000).unwrap(),
        };

        let order_book_3 = BookTickerEvent {
            sequence_id: 0,
            symbol: "ETHBTC".to_string(),
            price: Decimal::from_f64(0.02858000).unwrap(),
            qty: Decimal::from_f64(105.74550000).unwrap(),
        };

        let order_symbols = vec![
            OrderSymbol {
                symbol: "BTCUSDT".to_string(),
                symbol_order: SymbolOrder::Asc,
                min_profit_qty: Decimal::from_f64(0.0),
                max_order_qty: Decimal::from_f64(0.00030),
                order_book: &order_book_1,
                base_min_size: Default::default(),
                quote_min_size: Default::default(),
                base_max_size: Default::default(),
                quote_max_size: Default::default(),
                base_increment: Decimal::from_f64(0.00000001).unwrap(),
                quote_increment: Decimal::from_f64(0.00000001).unwrap(),
                price_increment: Decimal::from_f64(0.00000001).unwrap(),
            },
            OrderSymbol {
                symbol: "ETHUSDT".to_string(),
                symbol_order: SymbolOrder::Desc,
                min_profit_qty: None,
                max_order_qty: None,
                order_book: &order_book_2,
                base_min_size: Default::default(),
                quote_min_size: Default::default(),
                base_max_size: Default::default(),
                quote_max_size: Default::default(),
                base_increment: Decimal::from_f64(0.00000001).unwrap(),
                quote_increment: Decimal::from_f64(0.00000001).unwrap(),
                price_increment: Decimal::from_f64(0.00000001).unwrap(),
            },
            OrderSymbol {
                symbol: "ETHBTC".to_string(),
                symbol_order: SymbolOrder::Asc,
                min_profit_qty: None,
                max_order_qty: None,
                order_book: &order_book_3,
                base_min_size: Default::default(),
                quote_min_size: Default::default(),
                base_max_size: Default::default(),
                quote_max_size: Default::default(),
                base_increment: Decimal::from_f64(0.00000001).unwrap(),
                quote_increment: Decimal::from_f64(0.00000001).unwrap(),
                price_increment: Decimal::from_f64(0.00000001).unwrap(),
            },
        ];

        let orders =
            OrderBuilder::calculate_chain_profit(&order_symbols, market_depth_limit, fee_percent);

        assert_eq!(orders.len(), 3);

        assert_eq!(orders[0].symbol, "BTCUSDT");
        assert_eq!(orders[0].symbol_order, SymbolOrder::Asc);
        assert_eq!(orders[0].price.to_string(), "109615.46000000");
        assert_eq!(orders[0].base_qty.to_string(), "0.00020000");
        assert_eq!(orders[0].quote_qty.to_string(), "21.92309200");

        assert_eq!(orders[1].symbol, "ETHUSDT");
        assert_eq!(orders[1].symbol_order, SymbolOrder::Desc);
        assert_eq!(orders[1].price.to_string(), "2585.70000000");
        assert_eq!(orders[1].base_qty.to_string(), "21.92309200");
        assert_eq!(orders[1].quote_qty.to_string(), "0.00847859");

        assert_eq!(orders[2].symbol, "ETHBTC");
        assert_eq!(orders[2].symbol_order, SymbolOrder::Asc);
        assert_eq!(orders[2].price.to_string(), "0.02858000");
        assert_eq!(orders[2].base_qty.to_string(), "0.00847859");
        assert_eq!(orders[2].quote_qty.to_string(), "0.00024231");

        Ok(())
    }

    // Case #3: the 2nd pair of the 1st depth does not have enough volume to reach the volume
    // (order - ASC/DESC/ASC)
    #[tokio::test]
    async fn test_calculate_chain_profit_3() -> anyhow::Result<()> {
        let market_depth_limit: usize = 1;
        let fee_percent: Decimal = Decimal::from_str("0.075").unwrap();

        let order_book_1 = BookTickerEvent {
            sequence_id: 0,
            symbol: "BTCUSDT".to_string(),
            price: Decimal::from_f64(109615.46000000).unwrap(),
            qty: Decimal::from_f64(0.20000000).unwrap(),
        };

        let order_book_2 = BookTickerEvent {
            sequence_id: 0,
            symbol: "ETHUSDT".to_string(),
            price: Decimal::from_f64(1585.71000000).unwrap(),
            qty: Decimal::from_f64(0.0033).unwrap(),
        };

        let order_book_3 = BookTickerEvent {
            sequence_id: 0,
            symbol: "ETHBTC".to_string(),
            price: Decimal::from_f64(0.02858000).unwrap(),
            qty: Decimal::from_f64(105.74550000).unwrap(),
        };

        let order_symbols = vec![
            OrderSymbol {
                symbol: "BTCUSDT".to_string(),
                symbol_order: SymbolOrder::Asc,
                min_profit_qty: Decimal::from_f64(0.000030),
                max_order_qty: Decimal::from_f64(0.00030),
                order_book: &order_book_1,
                base_min_size: Default::default(),
                quote_min_size: Default::default(),
                base_max_size: Default::default(),
                quote_max_size: Default::default(),
                base_increment: Decimal::from_f64(0.00000001).unwrap(),
                quote_increment: Decimal::from_f64(0.00000001).unwrap(),
                price_increment: Decimal::from_f64(0.00000001).unwrap(),
            },
            OrderSymbol {
                symbol: "ETHUSDT".to_string(),
                symbol_order: SymbolOrder::Desc,
                min_profit_qty: None,
                max_order_qty: None,
                order_book: &order_book_2,
                base_min_size: Default::default(),
                quote_min_size: Default::default(),
                base_max_size: Default::default(),
                quote_max_size: Default::default(),
                base_increment: Decimal::from_f64(0.00000001).unwrap(),
                quote_increment: Decimal::from_f64(0.00000001).unwrap(),
                price_increment: Decimal::from_f64(0.00000001).unwrap(),
            },
            OrderSymbol {
                symbol: "ETHBTC".to_string(),
                symbol_order: SymbolOrder::Asc,
                min_profit_qty: None,
                max_order_qty: None,
                order_book: &order_book_3,
                base_min_size: Default::default(),
                quote_min_size: Default::default(),
                base_max_size: Default::default(),
                quote_max_size: Default::default(),
                base_increment: Decimal::from_f64(0.00000001).unwrap(),
                quote_increment: Decimal::from_f64(0.00000001).unwrap(),
                price_increment: Decimal::from_f64(0.00000001).unwrap(),
            },
        ];

        let orders =
            OrderBuilder::calculate_chain_profit(&order_symbols, market_depth_limit, fee_percent);

        assert_eq!(orders.len(), 3);

        assert_eq!(orders[0].symbol, "BTCUSDT");
        assert_eq!(orders[0].symbol_order, SymbolOrder::Asc);
        assert_eq!(orders[0].price.to_string(), "109615.46000000");
        assert_eq!(orders[0].base_qty.to_string(), "0.00004773");
        assert_eq!(orders[0].quote_qty.to_string(), "5.23194590");

        assert_eq!(orders[1].symbol, "ETHUSDT");
        assert_eq!(orders[1].symbol_order, SymbolOrder::Desc);
        assert_eq!(orders[1].price.to_string(), "1585.71000000");
        assert_eq!(orders[1].base_qty.to_string(), "5.23194590");
        assert_eq!(orders[1].quote_qty.to_string(), "0.00329943");

        assert_eq!(orders[2].symbol, "ETHBTC");
        assert_eq!(orders[2].symbol_order, SymbolOrder::Asc);
        assert_eq!(orders[2].price.to_string(), "0.02858000");
        assert_eq!(orders[2].base_qty.to_string(), "0.00329943");
        assert_eq!(orders[2].quote_qty.to_string(), "0.00009429");

        Ok(())
    }

    // Case #3: the 3rd pair of the 1st depth does not have enough volume to reach the volume
    // (order - ASC/DESC/ASC)
    #[tokio::test]
    async fn test_calculate_chain_profit_4() -> anyhow::Result<()> {
        let market_depth_limit: usize = 1;
        let fee_percent: Decimal = Decimal::from_str("0.075").unwrap();

        let order_book_1 = BookTickerEvent {
            sequence_id: 0,
            symbol: "BTCUSDT".to_string(),
            price: Decimal::from_f64(109615.46000000).unwrap(),
            qty: Decimal::from_f64(0.20000000).unwrap(),
        };

        let order_book_2 = BookTickerEvent {
            sequence_id: 0,
            symbol: "ETHUSDT".to_string(),
            price: Decimal::from_f64(2585.70000000).unwrap(),
            qty: Decimal::from_f64(19.28810000).unwrap(),
        };

        let order_book_3 = BookTickerEvent {
            sequence_id: 0,
            symbol: "ETHBTC".to_string(),
            price: Decimal::from_f64(0.02858000).unwrap(),
            qty: Decimal::from_f64(0.01).unwrap(),
        };

        let order_symbols = vec![
            OrderSymbol {
                symbol: "BTCUSDT".to_string(),
                symbol_order: SymbolOrder::Asc,
                min_profit_qty: Decimal::from_f64(0.000030),
                max_order_qty: Decimal::from_f64(0.00030),
                order_book: &order_book_1,
                base_min_size: Default::default(),
                quote_min_size: Default::default(),
                base_max_size: Default::default(),
                quote_max_size: Default::default(),
                base_increment: Decimal::from_f64(0.00000001).unwrap(),
                quote_increment: Decimal::from_f64(0.00000001).unwrap(),
                price_increment: Decimal::from_f64(0.00000001).unwrap(),
            },
            OrderSymbol {
                symbol: "ETHUSDT".to_string(),
                symbol_order: SymbolOrder::Desc,
                min_profit_qty: None,
                max_order_qty: None,
                order_book: &order_book_2,
                base_min_size: Default::default(),
                quote_min_size: Default::default(),
                base_max_size: Default::default(),
                quote_max_size: Default::default(),
                base_increment: Decimal::from_f64(0.00000001).unwrap(),
                quote_increment: Decimal::from_f64(0.00000001).unwrap(),
                price_increment: Decimal::from_f64(0.00000001).unwrap(),
            },
            OrderSymbol {
                symbol: "ETHBTC".to_string(),
                symbol_order: SymbolOrder::Asc,
                min_profit_qty: None,
                max_order_qty: None,
                order_book: &order_book_3,
                base_min_size: Default::default(),
                quote_min_size: Default::default(),
                base_max_size: Default::default(),
                quote_max_size: Default::default(),
                base_increment: Decimal::from_f64(0.00000001).unwrap(),
                quote_increment: Decimal::from_f64(0.00000001).unwrap(),
                price_increment: Decimal::from_f64(0.00000001).unwrap(),
            },
        ];

        let orders =
            OrderBuilder::calculate_chain_profit(&order_symbols, market_depth_limit, fee_percent);

        assert_eq!(orders.len(), 3);

        assert_eq!(orders[0].symbol, "BTCUSDT");
        assert_eq!(orders[0].symbol_order, SymbolOrder::Asc);
        assert_eq!(orders[0].price.to_string(), "109615.46000000");
        assert_eq!(orders[0].base_qty.to_string(), "0.00023588");
        assert_eq!(orders[0].quote_qty.to_string(), "25.85609470");

        assert_eq!(orders[1].symbol, "ETHUSDT");
        assert_eq!(orders[1].symbol_order, SymbolOrder::Desc);
        assert_eq!(orders[1].price.to_string(), "2585.70000000");
        assert_eq!(orders[1].base_qty.to_string(), "25.85609470");
        assert_eq!(orders[1].quote_qty.to_string(), "0.00999964");

        assert_eq!(orders[2].symbol, "ETHBTC");
        assert_eq!(orders[2].symbol_order, SymbolOrder::Asc);
        assert_eq!(orders[2].price.to_string(), "0.02858000");
        assert_eq!(orders[2].base_qty.to_string(), "0.00999964");
        assert_eq!(orders[2].quote_qty.to_string(), "0.00028578");

        Ok(())
    }

    // Case: skipped, does not pass the minimum quantity.
    #[tokio::test]
    async fn test_calculate_chain_profit_5() -> anyhow::Result<()> {
        let market_depth_limit: usize = 1;
        let fee_percent: Decimal = Decimal::from_str("0.075").unwrap();

        let order_book_1 = BookTickerEvent {
            sequence_id: 0,
            symbol: "ETHBTC".to_string(),
            price: Decimal::from_f64(0.03115000).unwrap(),
            qty: Decimal::from_f64(0.20000000).unwrap(),
        };

        let order_book_2 = BookTickerEvent {
            sequence_id: 0,
            symbol: "WBTCBTC".to_string(),
            price: Decimal::from_f64(0.99920000).unwrap(),
            qty: Decimal::from_f64(19.28810000).unwrap(),
        };

        let order_book_3 = BookTickerEvent {
            sequence_id: 0,
            symbol: "WBTCETH".to_string(),
            price: Decimal::from_f64(31.07000000).unwrap(),
            qty: Decimal::from_f64(1.5).unwrap(),
        };

        let order_symbols = vec![
            OrderSymbol {
                symbol: "ETHBTC".to_string(),
                symbol_order: SymbolOrder::Asc,
                min_profit_qty: Decimal::from_f64(0.0),
                max_order_qty: Decimal::from_f64(0.0079),
                order_book: &order_book_1,
                base_min_size: Default::default(),
                quote_min_size: Default::default(),
                base_max_size: Default::default(),
                quote_max_size: Default::default(),
                base_increment: Decimal::from_f64(0.00000001).unwrap(),
                quote_increment: Decimal::from_f64(0.00000001).unwrap(),
                price_increment: Decimal::from_f64(0.00000001).unwrap(),
            },
            OrderSymbol {
                symbol: "WBTCBTC".to_string(),
                symbol_order: SymbolOrder::Desc,
                min_profit_qty: None,
                max_order_qty: None,
                order_book: &order_book_2,
                base_min_size: Default::default(),
                quote_min_size: Default::default(),
                base_max_size: Default::default(),
                quote_max_size: Default::default(),
                base_increment: Decimal::from_f64(0.00000001).unwrap(),
                quote_increment: Decimal::from_f64(0.00000001).unwrap(),
                price_increment: Decimal::from_f64(0.00000001).unwrap(),
            },
            OrderSymbol {
                symbol: "WBTCETH".to_string(),
                symbol_order: SymbolOrder::Asc,
                min_profit_qty: None,
                max_order_qty: None,
                order_book: &order_book_3,
                base_min_size: Default::default(),
                quote_min_size: Default::default(),
                base_max_size: Default::default(),
                quote_max_size: Default::default(),
                base_increment: Decimal::from_f64(0.00000001).unwrap(),
                quote_increment: Decimal::from_f64(0.00000001).unwrap(),
                price_increment: Decimal::from_f64(0.00000001).unwrap(),
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
            sequence_id: 0,
            symbol: "ETHBTC".to_string(),
            price: Decimal::from_f64(0.03402000).unwrap(),
            qty: Decimal::from_f64(23.09700000).unwrap(),
        };

        let order_book_2 = BookTickerEvent {
            sequence_id: 0,
            symbol: "WBTCBTC".to_string(),
            price: Decimal::from_f64(0.00007820).unwrap(),
            qty: Decimal::from_f64(1.62000000).unwrap(),
        };

        let order_book_3 = BookTickerEvent {
            sequence_id: 0,
            symbol: "WBTCETH".to_string(),
            price: Decimal::from_f64(0.00243200).unwrap(),
            qty: Decimal::from_f64(0.54000000).unwrap(),
        };
        let order_symbols = vec![
            OrderSymbol {
                symbol: "ETHBTC".to_string(),
                symbol_order: SymbolOrder::Asc,
                min_profit_qty: Decimal::from_f64(0.0),
                max_order_qty: Decimal::from_f64(0.0079),
                order_book: &order_book_1,
                base_min_size: Default::default(),
                quote_min_size: Default::default(),
                base_max_size: Default::default(),
                quote_max_size: Default::default(),
                base_increment: Decimal::from_f64(0.00000001).unwrap(),
                quote_increment: Decimal::from_f64(0.00000001).unwrap(),
                price_increment: Decimal::from_f64(0.00000001).unwrap(),
            },
            OrderSymbol {
                symbol: "SSVBTC".to_string(),
                symbol_order: SymbolOrder::Desc,
                min_profit_qty: None,
                max_order_qty: None,
                order_book: &order_book_2,
                base_min_size: Default::default(),
                quote_min_size: Default::default(),
                base_max_size: Default::default(),
                quote_max_size: Default::default(),
                base_increment: Decimal::from_f64(0.00000001).unwrap(),
                quote_increment: Decimal::from_f64(0.00000001).unwrap(),
                price_increment: Decimal::from_f64(0.00000001).unwrap(),
            },
            OrderSymbol {
                symbol: "SSVETH".to_string(),
                symbol_order: SymbolOrder::Asc,
                min_profit_qty: None,
                max_order_qty: None,
                order_book: &order_book_3,
                base_min_size: Default::default(),
                quote_min_size: Default::default(),
                base_max_size: Default::default(),
                quote_max_size: Default::default(),
                base_increment: Decimal::from_f64(0.00000001).unwrap(),
                quote_increment: Decimal::from_f64(0.00000001).unwrap(),
                price_increment: Decimal::from_f64(0.00000001).unwrap(),
            },
        ];

        let orders =
            OrderBuilder::calculate_chain_profit(&order_symbols, market_depth_limit, fee_percent);
        assert_eq!(orders.len(), 3);

        assert_eq!(orders[0].symbol, "ETHBTC");
        assert_eq!(orders[0].symbol_order, SymbolOrder::Asc);
        assert_eq!(orders[0].price.to_string(), "0.03402000");
        assert_eq!(orders[0].base_qty.to_string(), "0.00124126");
        assert_eq!(orders[0].quote_qty.to_string(), "0.00004222");

        assert_eq!(orders[1].symbol, "SSVBTC");
        assert_eq!(orders[1].symbol_order, SymbolOrder::Desc);
        assert_eq!(orders[1].price.to_string(), "0.00007820");
        assert_eq!(orders[1].base_qty.to_string(), "0.00004222");
        assert_eq!(orders[1].quote_qty.to_string(), "0.53989769");

        assert_eq!(orders[2].symbol, "SSVETH");
        assert_eq!(orders[2].symbol_order, SymbolOrder::Asc);
        assert_eq!(orders[2].price.to_string(), "0.00243200");
        assert_eq!(orders[2].base_qty.to_string(), "0.53989769");
        assert_eq!(orders[2].quote_qty.to_string(), "0.00131303");

        Ok(())
    }
}
