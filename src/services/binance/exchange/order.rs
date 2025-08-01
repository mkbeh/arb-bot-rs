use std::{ops::Sub, sync::Arc, time::Duration};

use anyhow::bail;
use rust_decimal::{Decimal, prelude::Zero};
use tracing::error;
use uuid::Uuid;

use crate::{
    config::Asset,
    libs::{
        binance_api::{Filters, Market, OrderBook, OrderBookUnit},
        misc,
    },
    services::{
        Chain, Order,
        binance::{REQUEST_WEIGHT, exchange::ChainSymbol},
        enums::SymbolOrder,
        service::ORDERS_CHANNEL,
    },
};

#[derive(Clone, Debug)]
pub struct OrderSymbol {
    pub symbol: String,
    pub base_asset: String,
    pub base_asset_precision: u32,
    pub quote_asset: String,
    pub quote_precision: u32,
    pub symbol_order: SymbolOrder,
    pub min_profit_qty: Option<Decimal>,
    pub max_order_qty: Option<Decimal>,
    pub order_book: OrderBook,
    pub symbol_filter: SymbolFilter,
}

#[derive(Clone, Debug)]
pub struct LocalOrder {
    symbol: String,
    symbol_order: SymbolOrder,
    price: Decimal,
    base_qty: Decimal,
    base_precision: u32,
    quote_qty: Decimal,
    quote_precision: u32,
    symbol_filter: SymbolFilter,
}

#[derive(Clone, Debug, Default)]
pub struct SymbolFilter {
    lot_size_step: u32,
    tick_size: u32,
    lot_size_min_qty: Decimal,
}

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

    pub async fn build_chains_orders(
        self: Arc<Self>,
        chains: Vec<[ChainSymbol; 3]>,
        base_assets: Vec<Asset>,
    ) -> anyhow::Result<()> {
        let mut tasks = vec![];
        for chain in chains.iter() {
            tasks.push(tokio::spawn({
                let base_assets = base_assets.clone();
                let chain = chain.clone();
                let this = self.clone();
                async move {
                    if let Err(e) = this.build_orders(&base_assets, &chain).await {
                        error!("Failed to build orders for chain {:?}: {}", chain, e);
                    }
                }
            }));
        }

        for task in tasks {
            if let Err(e) = task.await {
                error!("Task failed to execute: {}", e);
            }
        }

        Ok(())
    }

    async fn build_orders(
        &self,
        base_assets: &[Asset],
        chain: &[ChainSymbol; 3],
    ) -> anyhow::Result<()> {
        let mut request_weight = REQUEST_WEIGHT.lock().await;

        // Calculate request weight, where api method 'get depth' cost 5 weight and api method
        // 'send orders' cost 1 weight - need x3 requests for each symbol.
        let weight = (5 + 1) * 3;

        while !request_weight.add(weight) {
            tokio::time::sleep(Duration::from_secs(60)).await;
        }

        // Async get order book for each symbol in chain.
        let tasks: Vec<_> = chain
            .clone()
            .into_iter()
            .map(|wrapper| {
                let client = self.market_api.clone();
                let depth_limit = self.market_depth_limit;
                tokio::spawn(async move {
                    client
                        .get_depth(wrapper.symbol.symbol.clone(), depth_limit)
                        .await
                })
            })
            .collect();

        let mut order_books = vec![];

        for task in tasks {
            match task.await? {
                Ok(order_book) => order_books.push(order_book),
                Err(e) => bail!("failed to get symbol order book: {}", e),
            }
        }

        // Build orders info and calculate profit.
        let mut order_symbols = vec![];

        for (i, chain_symbol) in chain.iter().enumerate() {
            // Define limits for 1st pair.
            let mut min_profit_qty = None;
            let mut max_order_qty = None;

            if i == 0 {
                let base_asset = match find_base_asset(base_assets, chain_symbol) {
                    Some(base) => base,
                    _ => bail!(
                        "failed to find base asset for symbol {}",
                        chain_symbol.symbol.symbol
                    ),
                };

                min_profit_qty = Some(base_asset.min_profit_qty);
                max_order_qty = Some(base_asset.max_order_qty);
            }

            let symbol = &chain_symbol.symbol;
            let symbol_filter = define_symbol_filter(&symbol.filters);

            let order_symbol = OrderSymbol {
                symbol: symbol.symbol.clone(),
                base_asset: symbol.base_asset.clone(),
                base_asset_precision: symbol.base_asset_precision,
                quote_asset: symbol.quote_asset.clone(),
                quote_precision: symbol.quote_precision,
                symbol_order: chain_symbol.order,
                min_profit_qty,
                max_order_qty,
                order_book: order_books[i].clone(),
                symbol_filter,
            };

            order_symbols.push(order_symbol);
        }

        let orders = self.calculate_chain_profit(&order_symbols);
        if orders.is_empty() {
            // Sub reserved 1x3 weight for send orders.
            request_weight.sub(3);
            return Ok(());
        }

        let msg = Chain {
            ts: misc::time::get_current_timestamp(),
            chain_id: Uuid::new_v4(),
            orders,
        };
        ORDERS_CHANNEL.tx.send(msg).await?;

        Ok(())
    }

    fn calculate_chain_profit(&self, order_symbols: &[OrderSymbol]) -> Vec<Order> {
        // Recalculate quantities of orders. First order in chain always skip, because operate
        // with a max order quantity value.
        let recalculate_orders_qty_fn = |orders: &mut Vec<LocalOrder>, order: usize| {
            let orders_count = orders.len();
            let mut count = 1;

            while count <= order {
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
        };

        let mut orders: Vec<LocalOrder> = vec![];
        let mut depth_limit = 0;
        let max_order_qty = get_max_order_qty(order_symbols.first().unwrap());

        while depth_limit < self.market_depth_limit {
            for (i, order_symbol) in order_symbols.iter().enumerate() {
                // Define list of orders according to the order of assets in symbol.
                let order_units: &Vec<OrderBookUnit> = match order_symbol.symbol_order {
                    SymbolOrder::Asc => order_symbol.order_book.bids.as_ref(),
                    SymbolOrder::Desc => order_symbol.order_book.asks.as_ref(),
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

                for order_unit in order_units.iter().take(depth_limit + 1) {
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

                orders.push(LocalOrder {
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
                    recalculate_orders_qty_fn(&mut orders, i);
                }
            }

            // Compare first chain order qty and first chain item qty limit.
            // If it is equal, there is no point in trying to sum up the qty, so break.
            if orders[orders.len() - order_symbols.len()].base_qty == max_order_qty {
                break;
            }

            depth_limit += 1;
        }

        // Round and recalculate quantities according to binance api rules.
        let mut profit_orders = vec![];
        let mut min_profit_qty = get_min_profit_qty(order_symbols.first().unwrap());

        // Iterate over every first order in chain.
        'outer_loop: for i in (0..).take(orders.len() - 1).step_by(order_symbols.len()) {
            let mut count = 0;
            let mut tmp_orders: Vec<Order> = vec![];

            while count < order_symbols.len() {
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

                tmp_orders.push(Order {
                    symbol: orders[count].symbol.clone(),
                    symbol_order: orders[count].symbol_order,
                    price,
                    base_qty: rounded_base_qty,
                    quote_qty: rounded_quote_qty,
                });

                count += 1;
            }

            // Check profit.
            //
            // Difference between the outbound volume of the last symbol in chain and the inbound
            // volume of the first symbol in chain.
            let diff_qty =
                tmp_orders.last().unwrap().quote_qty - tmp_orders.first().unwrap().base_qty;

            if diff_qty >= min_profit_qty {
                min_profit_qty = diff_qty;
                profit_orders.extend_from_slice(&tmp_orders);
            }
        }

        // Return 3 last profit orders.
        if profit_orders.len() >= order_symbols.len() {
            let idx = profit_orders.len().sub(order_symbols.len());
            profit_orders[idx..].to_vec()
        } else {
            profit_orders
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
            _ => continue,
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

#[cfg(test)]
mod tests {
    use mockito::{Matcher, Server};
    use rust_decimal::prelude::FromPrimitive;

    use super::*;
    use crate::{
        libs::{
            binance_api,
            binance_api::{Binance, OrderBookUnit, Symbol},
        },
        services::enums::SymbolOrder,
    };

    #[tokio::test]
    async fn test_build_chains_orders() -> anyhow::Result<()> {
        let mut server = Server::new_async().await;

        let payload_btcusdt = r#"
        {
          "lastUpdateId": 72224518924,
          "bids": [
            [
              "109615.46000000",
              "7.27795000"
            ],
            [
              "109614.96000000",
              "0.00046000"
            ],
            [
              "109614.48000000",
              "0.05832000"
            ],
            [
              "109614.20000000",
              "0.73748000"
            ],
            [
              "109614.07000000",
              "0.00068000"
            ]
          ],
          "asks": [
            [
              "109615.47000000",
              "2.22969000"
            ],
            [
              "109615.48000000",
              "0.00028000"
            ],
            [
              "109615.99000000",
              "0.00116000"
            ],
            [
              "109616.61000000",
              "0.00005000"
            ],
            [
              "109617.67000000",
              "0.00050000"
            ]
          ]
        }
        "#;

        let payload_ethusdt = r#"
        {
          "lastUpdateId": 54622041690,
          "bids": [
            [
              "2585.70000000",
              "14.64600000"
            ],
            [
              "2585.69000000",
              "0.00210000"
            ],
            [
              "2585.67000000",
              "0.00510000"
            ],
            [
              "2585.66000000",
              "0.00440000"
            ],
            [
              "2585.65000000",
              "0.00210000"
            ]
          ],
          "asks": [
            [
              "2585.71000000",
              "19.28810000"
            ],
            [
              "2585.72000000",
              "0.40280000"
            ],
            [
              "2585.73000000",
              "0.00440000"
            ],
            [
              "2585.77000000",
              "0.00440000"
            ],
            [
              "2585.79000000",
              "0.00210000"
            ]
          ]
        }
        "#;

        let payload_ethbtc = r#"
        {
          "lastUpdateId": 8215337504,
          "bids": [
            [
              "0.02358000",
              "105.74550000"
            ],
            [
              "0.02357000",
              "57.30640000"
            ],
            [
              "0.02356000",
              "96.84260000"
            ],
            [
              "0.02355000",
              "93.05990000"
            ],
            [
              "0.02354000",
              "66.95170000"
            ]
          ],
          "asks": [
            [
              "0.02359000",
              "25.63400000"
            ],
            [
              "0.02360000",
              "53.22680000"
            ],
            [
              "0.02361000",
              "81.91300000"
            ],
            [
              "0.02362000",
              "59.61190000"
            ],
            [
              "0.02363000",
              "86.74020000"
            ]
          ]
        }
        "#;

        let mock_order_book_btcusdt = server
            .mock("GET", "/api/v3/depth")
            .with_header("content-type", "application/json;charset=UTF-8")
            .match_query(Matcher::Regex("symbol=BTCUSDT&limit=5".into()))
            .with_body(payload_btcusdt)
            .create_async();

        let mock_order_book_ethusdt = server
            .mock("GET", "/api/v3/depth")
            .with_header("content-type", "application/json;charset=UTF-8")
            .match_query(Matcher::Regex("symbol=ETHUSDT&limit=5".into()))
            .with_body(payload_ethusdt)
            .create_async();

        let mock_order_book_ethbtc = server
            .mock("GET", "/api/v3/depth")
            .with_header("content-type", "application/json;charset=UTF-8")
            .match_query(Matcher::Regex("symbol=ETHBTC&limit=5".into()))
            .with_body(payload_ethbtc)
            .create_async();

        let (mock_order_book_ethbtc, mock_order_book_ltcbtc, mock_order_book_ltceth) = futures::join!(
            mock_order_book_btcusdt,
            mock_order_book_ethusdt,
            mock_order_book_ethbtc
        );

        let test_chains = vec![[
            ChainSymbol {
                symbol: Symbol {
                    symbol: "BTCUSDT".to_owned(),
                    base_asset: "BTC".to_owned(),
                    base_asset_precision: 8,
                    quote_asset: "USDT".to_owned(),
                    quote_precision: 8,
                    filters: vec![
                        Filters::PriceFilter {
                            min_price: Default::default(),
                            max_price: Default::default(),
                            tick_size: Decimal::from_f64(0.01000000).unwrap(),
                        },
                        Filters::LotSize {
                            min_qty: Decimal::from_f64(0.00001000).unwrap(),
                            max_qty: Default::default(),
                            step_size: Decimal::from_f64(0.00001000).unwrap(),
                        },
                    ],
                    ..Default::default()
                },
                order: SymbolOrder::Asc,
            },
            ChainSymbol {
                symbol: Symbol {
                    symbol: "ETHUSDT".to_owned(),
                    base_asset: "ETH".to_owned(),
                    base_asset_precision: 8,
                    quote_asset: "USDT".to_owned(),
                    quote_precision: 8,
                    filters: vec![
                        Filters::PriceFilter {
                            min_price: Default::default(),
                            max_price: Default::default(),
                            tick_size: Decimal::from_f64(0.01000000).unwrap(),
                        },
                        Filters::LotSize {
                            min_qty: Decimal::from_f64(0.00010000).unwrap(),
                            max_qty: Default::default(),
                            step_size: Decimal::from_f64(0.00010000).unwrap(),
                        },
                    ],
                    ..Default::default()
                },
                order: SymbolOrder::Desc,
            },
            ChainSymbol {
                symbol: Symbol {
                    symbol: "ETHBTC".to_owned(),
                    base_asset: "ETH".to_owned(),
                    base_asset_precision: 8,
                    quote_asset: "BTC".to_owned(),
                    quote_precision: 8,
                    filters: vec![
                        Filters::PriceFilter {
                            min_price: Default::default(),
                            max_price: Default::default(),
                            tick_size: Decimal::from_f64(0.00001000).unwrap(),
                        },
                        Filters::LotSize {
                            min_qty: Decimal::from_f64(0.00010000).unwrap(),
                            max_qty: Default::default(),
                            step_size: Decimal::from_f64(0.00010000).unwrap(),
                        },
                    ],
                    ..Default::default()
                },
                order: SymbolOrder::Asc,
            },
        ]];

        let base_assets: Vec<Asset> = vec![
            Asset {
                asset: "BTC".to_string(),
                symbol: Some("BTCUSDT".to_owned()),
                min_profit_qty: Decimal::from_f64(0.000030).unwrap(),
                max_order_qty: Decimal::from_f64(0.00030).unwrap(),
            },
            Asset {
                asset: "ETH".to_string(),
                symbol: Some("ETHUSDT".to_owned()),
                min_profit_qty: Decimal::from_f64(0.0012).unwrap(),
                max_order_qty: Decimal::from_f64(0.012).unwrap(),
            },
            Asset {
                asset: "USDT".to_string(),
                symbol: Some("USDT".to_owned()),
                min_profit_qty: Decimal::from_f64(3.0).unwrap(),
                max_order_qty: Decimal::from_f64(30.0).unwrap(),
            },
        ];

        {
            let mut request_weight = REQUEST_WEIGHT.lock().await;
            request_weight.set_weight_limit(5000);
        }

        let api_config = binance_api::Config {
            api_url: server.url(),
            ..Default::default()
        };

        let market_api = match Binance::new(api_config.clone()) {
            Ok(v) => v,
            Err(e) => bail!("Failed init binance client: {e}"),
        };

        let orders_builder = Arc::new(OrderBuilder::new(market_api, 5));
        let result = orders_builder
            .build_chains_orders(test_chains, base_assets)
            .await;

        assert!(result.is_ok());

        mock_order_book_ethbtc.assert_async().await;
        mock_order_book_ltcbtc.assert_async().await;
        mock_order_book_ltceth.assert_async().await;

        Ok(())
    }

    // Case #1: all orders of the 1st depth have volumes greater than the volume limit.
    // (order - ASC/DESC/ASC)
    #[tokio::test]
    async fn test_calculate_chain_profit_1() -> anyhow::Result<()> {
        let market_api = match Binance::new(binance_api::Config::default()) {
            Ok(v) => v,
            Err(e) => bail!("Failed init binance client: {e}"),
        };

        let order_builder = OrderBuilder::new(market_api, 3);

        let order_symbols = vec![
            OrderSymbol {
                symbol: "BTCUSDT".to_string(),
                base_asset: "BTC".to_string(),
                base_asset_precision: 8,
                quote_asset: "USDT".to_string(),
                quote_precision: 8,
                symbol_order: SymbolOrder::Asc,
                min_profit_qty: Decimal::from_f64(0.000030),
                max_order_qty: Decimal::from_f64(0.00030),
                order_book: OrderBook {
                    last_update_id: 1,
                    bids: vec![
                        OrderBookUnit {
                            price: Decimal::from_f64(109615.46000000).unwrap(),
                            qty: Decimal::from_f64(7.27795000).unwrap(),
                        },
                        OrderBookUnit {
                            price: Decimal::from_f64(109614.96000000).unwrap(),
                            qty: Decimal::from_f64(0.00046000).unwrap(),
                        },
                        OrderBookUnit {
                            price: Decimal::from_f64(109614.48000000).unwrap(),
                            qty: Decimal::from_f64(0.05832000).unwrap(),
                        },
                    ],
                    asks: vec![
                        OrderBookUnit {
                            price: Decimal::from_f64(109615.47000000).unwrap(),
                            qty: Decimal::from_f64(2.22969000).unwrap(),
                        },
                        OrderBookUnit {
                            price: Decimal::from_f64(109615.48000000).unwrap(),
                            qty: Decimal::from_f64(0.00028000).unwrap(),
                        },
                        OrderBookUnit {
                            price: Decimal::from_f64(109615.99000000).unwrap(),
                            qty: Decimal::from_f64(0.00116000).unwrap(),
                        },
                    ],
                },
                symbol_filter: SymbolFilter {
                    lot_size_step: 5,
                    tick_size: 2,
                    lot_size_min_qty: Decimal::from_f64(0.00001000).unwrap(),
                },
            },
            OrderSymbol {
                symbol: "ETHUSDT".to_string(),
                base_asset: "ETH".to_string(),
                base_asset_precision: 8,
                quote_asset: "USDT".to_string(),
                quote_precision: 8,
                symbol_order: SymbolOrder::Desc,
                min_profit_qty: None,
                max_order_qty: None,
                order_book: OrderBook {
                    last_update_id: 1,
                    bids: vec![
                        OrderBookUnit {
                            price: Decimal::from_f64(2585.70000000).unwrap(),
                            qty: Decimal::from_f64(14.64600000).unwrap(),
                        },
                        OrderBookUnit {
                            price: Decimal::from_f64(2585.69000000).unwrap(),
                            qty: Decimal::from_f64(0.00210000).unwrap(),
                        },
                        OrderBookUnit {
                            price: Decimal::from_f64(2585.67000000).unwrap(),
                            qty: Decimal::from_f64(0.00510000).unwrap(),
                        },
                    ],
                    asks: vec![
                        OrderBookUnit {
                            price: Decimal::from_f64(2585.71000000).unwrap(),
                            qty: Decimal::from_f64(19.28810000).unwrap(),
                        },
                        OrderBookUnit {
                            price: Decimal::from_f64(2585.72000000).unwrap(),
                            qty: Decimal::from_f64(0.40280000).unwrap(),
                        },
                        OrderBookUnit {
                            price: Decimal::from_f64(2585.73000000).unwrap(),
                            qty: Decimal::from_f64(0.00440000).unwrap(),
                        },
                    ],
                },
                symbol_filter: SymbolFilter {
                    lot_size_step: 4,
                    tick_size: 2,
                    lot_size_min_qty: Decimal::from_f64(0.00010000).unwrap(),
                },
            },
            OrderSymbol {
                symbol: "ETHBTC".to_string(),
                base_asset: "ETH".to_string(),
                base_asset_precision: 8,
                quote_asset: "BTC".to_string(),
                quote_precision: 8,
                symbol_order: SymbolOrder::Asc,
                min_profit_qty: None,
                max_order_qty: None,
                order_book: OrderBook {
                    last_update_id: 1,
                    bids: vec![
                        OrderBookUnit {
                            price: Decimal::from_f64(0.02858000).unwrap(),
                            qty: Decimal::from_f64(105.74550000).unwrap(),
                        },
                        OrderBookUnit {
                            price: Decimal::from_f64(0.02357000).unwrap(),
                            qty: Decimal::from_f64(57.30640000).unwrap(),
                        },
                        OrderBookUnit {
                            price: Decimal::from_f64(0.02356000).unwrap(),
                            qty: Decimal::from_f64(96.84260000).unwrap(),
                        },
                    ],
                    asks: vec![
                        OrderBookUnit {
                            price: Decimal::from_f64(0.02359000).unwrap(),
                            qty: Decimal::from_f64(25.63400000).unwrap(),
                        },
                        OrderBookUnit {
                            price: Decimal::from_f64(0.02360000).unwrap(),
                            qty: Decimal::from_f64(53.22680000).unwrap(),
                        },
                        OrderBookUnit {
                            price: Decimal::from_f64(0.02361000).unwrap(),
                            qty: Decimal::from_f64(81.91300000).unwrap(),
                        },
                    ],
                },
                symbol_filter: SymbolFilter {
                    lot_size_step: 4,
                    tick_size: 5,
                    lot_size_min_qty: Decimal::from_f64(0.00010000).unwrap(),
                },
            },
        ];

        let orders = order_builder.calculate_chain_profit(&order_symbols);

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
        let market_api = match Binance::new(binance_api::Config::default()) {
            Ok(v) => v,
            Err(e) => bail!("Failed init binance client: {e}"),
        };

        let order_builder = OrderBuilder::new(market_api, 3);

        let order_symbols = vec![
            OrderSymbol {
                symbol: "BTCUSDT".to_string(),
                base_asset: "BTC".to_string(),
                base_asset_precision: 8,
                quote_asset: "USDT".to_string(),
                quote_precision: 8,
                symbol_order: SymbolOrder::Asc,
                min_profit_qty: Decimal::from_f64(0.0),
                max_order_qty: Decimal::from_f64(0.00030),
                order_book: OrderBook {
                    last_update_id: 1,
                    bids: vec![
                        OrderBookUnit {
                            price: Decimal::from_f64(109615.46000000).unwrap(),
                            qty: Decimal::from_f64(0.00020000).unwrap(), // <---- here
                        },
                        OrderBookUnit {
                            price: Decimal::from_f64(109614.96000000).unwrap(),
                            qty: Decimal::from_f64(0.00046000).unwrap(),
                        },
                        OrderBookUnit {
                            price: Decimal::from_f64(109614.48000000).unwrap(),
                            qty: Decimal::from_f64(0.05832000).unwrap(),
                        },
                    ],
                    asks: vec![
                        OrderBookUnit {
                            price: Decimal::from_f64(109615.47000000).unwrap(),
                            qty: Decimal::from_f64(2.22969000).unwrap(),
                        },
                        OrderBookUnit {
                            price: Decimal::from_f64(109615.48000000).unwrap(),
                            qty: Decimal::from_f64(0.00028000).unwrap(),
                        },
                        OrderBookUnit {
                            price: Decimal::from_f64(109615.99000000).unwrap(),
                            qty: Decimal::from_f64(0.00116000).unwrap(),
                        },
                    ],
                },
                symbol_filter: SymbolFilter {
                    lot_size_step: 5,
                    tick_size: 2,
                    lot_size_min_qty: Decimal::from_f64(0.00001000).unwrap(),
                },
            },
            OrderSymbol {
                symbol: "ETHUSDT".to_string(),
                base_asset: "ETH".to_string(),
                base_asset_precision: 8,
                quote_asset: "USDT".to_string(),
                quote_precision: 8,
                symbol_order: SymbolOrder::Desc,
                min_profit_qty: None,
                max_order_qty: None,
                order_book: OrderBook {
                    last_update_id: 1,
                    bids: vec![
                        OrderBookUnit {
                            price: Decimal::from_f64(2585.70000000).unwrap(),
                            qty: Decimal::from_f64(14.64600000).unwrap(),
                        },
                        OrderBookUnit {
                            price: Decimal::from_f64(2585.69000000).unwrap(),
                            qty: Decimal::from_f64(0.00210000).unwrap(),
                        },
                        OrderBookUnit {
                            price: Decimal::from_f64(2585.67000000).unwrap(),
                            qty: Decimal::from_f64(0.00510000).unwrap(),
                        },
                    ],
                    asks: vec![
                        OrderBookUnit {
                            price: Decimal::from_f64(2585.71000000).unwrap(),
                            qty: Decimal::from_f64(19.28810000).unwrap(),
                        },
                        OrderBookUnit {
                            price: Decimal::from_f64(2585.72000000).unwrap(),
                            qty: Decimal::from_f64(0.40280000).unwrap(),
                        },
                        OrderBookUnit {
                            price: Decimal::from_f64(2585.73000000).unwrap(),
                            qty: Decimal::from_f64(0.00440000).unwrap(),
                        },
                    ],
                },
                symbol_filter: SymbolFilter {
                    lot_size_step: 4,
                    tick_size: 2,
                    lot_size_min_qty: Decimal::from_f64(0.00010000).unwrap(),
                },
            },
            OrderSymbol {
                symbol: "ETHBTC".to_string(),
                base_asset: "ETH".to_string(),
                base_asset_precision: 8,
                quote_asset: "BTC".to_string(),
                quote_precision: 8,
                symbol_order: SymbolOrder::Asc,
                min_profit_qty: None,
                max_order_qty: None,
                order_book: OrderBook {
                    last_update_id: 1,
                    bids: vec![
                        OrderBookUnit {
                            price: Decimal::from_f64(0.02858000).unwrap(),
                            qty: Decimal::from_f64(105.74550000).unwrap(),
                        },
                        OrderBookUnit {
                            price: Decimal::from_f64(0.02357000).unwrap(),
                            qty: Decimal::from_f64(57.30640000).unwrap(),
                        },
                        OrderBookUnit {
                            price: Decimal::from_f64(0.02356000).unwrap(),
                            qty: Decimal::from_f64(96.84260000).unwrap(),
                        },
                    ],
                    asks: vec![
                        OrderBookUnit {
                            price: Decimal::from_f64(0.02359000).unwrap(),
                            qty: Decimal::from_f64(25.63400000).unwrap(),
                        },
                        OrderBookUnit {
                            price: Decimal::from_f64(0.02360000).unwrap(),
                            qty: Decimal::from_f64(53.22680000).unwrap(),
                        },
                        OrderBookUnit {
                            price: Decimal::from_f64(0.02361000).unwrap(),
                            qty: Decimal::from_f64(81.91300000).unwrap(),
                        },
                    ],
                },
                symbol_filter: SymbolFilter {
                    lot_size_step: 4,
                    tick_size: 5,
                    lot_size_min_qty: Decimal::from_f64(0.00010000).unwrap(),
                },
            },
        ];

        let orders = order_builder.calculate_chain_profit(&order_symbols);

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

    // Case #3: the 2nd pair of the 1st depth does not have enough volume to reach the volume limit.
    // (order - ASC/DESC/ASC)
    #[tokio::test]
    async fn test_calculate_chain_profit_3() -> anyhow::Result<()> {
        let market_api = match Binance::new(binance_api::Config::default()) {
            Ok(v) => v,
            Err(e) => bail!("Failed init binance client: {e}"),
        };

        let order_builder = OrderBuilder::new(market_api, 3);

        let order_symbols = vec![
            OrderSymbol {
                symbol: "BTCUSDT".to_string(),
                base_asset: "BTC".to_string(),
                base_asset_precision: 8,
                quote_asset: "USDT".to_string(),
                quote_precision: 8,
                symbol_order: SymbolOrder::Asc,
                min_profit_qty: Decimal::from_f64(0.000030),
                max_order_qty: Decimal::from_f64(0.00030),
                order_book: OrderBook {
                    last_update_id: 1,
                    bids: vec![
                        OrderBookUnit {
                            price: Decimal::from_f64(109615.46000000).unwrap(),
                            qty: Decimal::from_f64(0.20000000).unwrap(),
                        },
                        OrderBookUnit {
                            price: Decimal::from_f64(109614.96000000).unwrap(),
                            qty: Decimal::from_f64(0.00046000).unwrap(),
                        },
                        OrderBookUnit {
                            price: Decimal::from_f64(109614.48000000).unwrap(),
                            qty: Decimal::from_f64(0.05832000).unwrap(),
                        },
                    ],
                    asks: vec![
                        OrderBookUnit {
                            price: Decimal::from_f64(109615.47000000).unwrap(),
                            qty: Decimal::from_f64(2.22969000).unwrap(),
                        },
                        OrderBookUnit {
                            price: Decimal::from_f64(109615.48000000).unwrap(),
                            qty: Decimal::from_f64(0.00028000).unwrap(),
                        },
                        OrderBookUnit {
                            price: Decimal::from_f64(109615.99000000).unwrap(),
                            qty: Decimal::from_f64(0.00116000).unwrap(),
                        },
                    ],
                },
                symbol_filter: SymbolFilter {
                    lot_size_step: 5,
                    tick_size: 2,
                    lot_size_min_qty: Decimal::from_f64(0.00001000).unwrap(),
                },
            },
            OrderSymbol {
                symbol: "ETHUSDT".to_string(),
                base_asset: "ETH".to_string(),
                base_asset_precision: 8,
                quote_asset: "USDT".to_string(),
                quote_precision: 8,
                symbol_order: SymbolOrder::Desc,
                min_profit_qty: None,
                max_order_qty: None,
                order_book: OrderBook {
                    last_update_id: 1,
                    bids: vec![
                        OrderBookUnit {
                            price: Decimal::from_f64(2585.70000000).unwrap(),
                            qty: Decimal::from_f64(19.28810000).unwrap(),
                        },
                        OrderBookUnit {
                            price: Decimal::from_f64(2585.69000000).unwrap(),
                            qty: Decimal::from_f64(0.00210000).unwrap(),
                        },
                        OrderBookUnit {
                            price: Decimal::from_f64(2585.67000000).unwrap(),
                            qty: Decimal::from_f64(0.00510000).unwrap(),
                        },
                    ],
                    asks: vec![
                        OrderBookUnit {
                            price: Decimal::from_f64(2585.71000000).unwrap(),
                            qty: Decimal::from_f64(0.0033).unwrap(), // <---- here
                        },
                        OrderBookUnit {
                            price: Decimal::from_f64(2585.72000000).unwrap(),
                            qty: Decimal::from_f64(0.40280000).unwrap(),
                        },
                        OrderBookUnit {
                            price: Decimal::from_f64(2585.73000000).unwrap(),
                            qty: Decimal::from_f64(0.00440000).unwrap(),
                        },
                    ],
                },
                symbol_filter: SymbolFilter {
                    lot_size_step: 4,
                    tick_size: 2,
                    lot_size_min_qty: Decimal::from_f64(0.00010000).unwrap(),
                },
            },
            OrderSymbol {
                symbol: "ETHBTC".to_string(),
                base_asset: "ETH".to_string(),
                base_asset_precision: 8,
                quote_asset: "BTC".to_string(),
                quote_precision: 8,
                symbol_order: SymbolOrder::Asc,
                min_profit_qty: None,
                max_order_qty: None,
                order_book: OrderBook {
                    last_update_id: 1,
                    bids: vec![
                        OrderBookUnit {
                            price: Decimal::from_f64(0.02858000).unwrap(),
                            qty: Decimal::from_f64(105.74550000).unwrap(),
                        },
                        OrderBookUnit {
                            price: Decimal::from_f64(0.02357000).unwrap(),
                            qty: Decimal::from_f64(57.30640000).unwrap(),
                        },
                        OrderBookUnit {
                            price: Decimal::from_f64(0.02356000).unwrap(),
                            qty: Decimal::from_f64(96.84260000).unwrap(),
                        },
                    ],
                    asks: vec![
                        OrderBookUnit {
                            price: Decimal::from_f64(0.02359000).unwrap(),
                            qty: Decimal::from_f64(25.63400000).unwrap(),
                        },
                        OrderBookUnit {
                            price: Decimal::from_f64(0.02360000).unwrap(),
                            qty: Decimal::from_f64(53.22680000).unwrap(),
                        },
                        OrderBookUnit {
                            price: Decimal::from_f64(0.02361000).unwrap(),
                            qty: Decimal::from_f64(81.91300000).unwrap(),
                        },
                    ],
                },
                symbol_filter: SymbolFilter {
                    lot_size_step: 4,
                    tick_size: 5,
                    lot_size_min_qty: Decimal::from_f64(0.00010000).unwrap(),
                },
            },
        ];

        let orders = order_builder.calculate_chain_profit(&order_symbols);

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

    // Case #3: the 3rd pair of the 1st depth does not have enough volume to reach the volume limit.
    // (order - ASC/DESC/ASC)
    #[tokio::test]
    async fn test_calculate_chain_profit_4() -> anyhow::Result<()> {
        let market_api = match Binance::new(binance_api::Config::default()) {
            Ok(v) => v,
            Err(e) => bail!("Failed init binance client: {e}"),
        };

        let order_builder = OrderBuilder::new(market_api, 3);

        let order_symbols = vec![
            OrderSymbol {
                symbol: "BTCUSDT".to_string(),
                base_asset: "BTC".to_string(),
                base_asset_precision: 8,
                quote_asset: "USDT".to_string(),
                quote_precision: 8,
                symbol_order: SymbolOrder::Asc,
                min_profit_qty: Decimal::from_f64(0.000030),
                max_order_qty: Decimal::from_f64(0.00030),
                order_book: OrderBook {
                    last_update_id: 1,
                    bids: vec![
                        OrderBookUnit {
                            price: Decimal::from_f64(109615.46000000).unwrap(),
                            qty: Decimal::from_f64(0.20000000).unwrap(),
                        },
                        OrderBookUnit {
                            price: Decimal::from_f64(109614.96000000).unwrap(),
                            qty: Decimal::from_f64(0.00046000).unwrap(),
                        },
                        OrderBookUnit {
                            price: Decimal::from_f64(109614.48000000).unwrap(),
                            qty: Decimal::from_f64(0.05832000).unwrap(),
                        },
                    ],
                    asks: vec![
                        OrderBookUnit {
                            price: Decimal::from_f64(109615.47000000).unwrap(),
                            qty: Decimal::from_f64(2.22969000).unwrap(),
                        },
                        OrderBookUnit {
                            price: Decimal::from_f64(109615.48000000).unwrap(),
                            qty: Decimal::from_f64(0.00028000).unwrap(),
                        },
                        OrderBookUnit {
                            price: Decimal::from_f64(109615.99000000).unwrap(),
                            qty: Decimal::from_f64(0.00116000).unwrap(),
                        },
                    ],
                },
                symbol_filter: SymbolFilter {
                    lot_size_step: 5,
                    tick_size: 2,
                    lot_size_min_qty: Decimal::from_f64(0.00001000).unwrap(),
                },
            },
            OrderSymbol {
                symbol: "ETHUSDT".to_string(),
                base_asset: "ETH".to_string(),
                base_asset_precision: 8,
                quote_asset: "USDT".to_string(),
                quote_precision: 8,
                symbol_order: SymbolOrder::Desc,
                min_profit_qty: None,
                max_order_qty: None,
                order_book: OrderBook {
                    last_update_id: 1,
                    bids: vec![
                        OrderBookUnit {
                            price: Decimal::from_f64(2585.70000000).unwrap(),
                            qty: Decimal::from_f64(19.28810000).unwrap(),
                        },
                        OrderBookUnit {
                            price: Decimal::from_f64(2585.69000000).unwrap(),
                            qty: Decimal::from_f64(0.00210000).unwrap(),
                        },
                        OrderBookUnit {
                            price: Decimal::from_f64(2585.67000000).unwrap(),
                            qty: Decimal::from_f64(0.00510000).unwrap(),
                        },
                    ],
                    asks: vec![
                        OrderBookUnit {
                            price: Decimal::from_f64(2585.71000000).unwrap(),
                            qty: Decimal::from_f64(0.9).unwrap(),
                        },
                        OrderBookUnit {
                            price: Decimal::from_f64(2585.72000000).unwrap(),
                            qty: Decimal::from_f64(0.40280000).unwrap(),
                        },
                        OrderBookUnit {
                            price: Decimal::from_f64(2585.73000000).unwrap(),
                            qty: Decimal::from_f64(0.00440000).unwrap(),
                        },
                    ],
                },
                symbol_filter: SymbolFilter {
                    lot_size_step: 4,
                    tick_size: 2,
                    lot_size_min_qty: Decimal::from_f64(0.00010000).unwrap(),
                },
            },
            OrderSymbol {
                symbol: "ETHBTC".to_string(),
                base_asset: "ETH".to_string(),
                base_asset_precision: 8,
                quote_asset: "BTC".to_string(),
                quote_precision: 8,
                symbol_order: SymbolOrder::Asc,
                min_profit_qty: None,
                max_order_qty: None,
                order_book: OrderBook {
                    last_update_id: 1,
                    bids: vec![
                        OrderBookUnit {
                            price: Decimal::from_f64(0.02858000).unwrap(),
                            qty: Decimal::from_f64(0.01).unwrap(), // <---- here
                        },
                        OrderBookUnit {
                            price: Decimal::from_f64(0.02857000).unwrap(),
                            qty: Decimal::from_f64(57.30640000).unwrap(),
                        },
                        OrderBookUnit {
                            price: Decimal::from_f64(0.02356000).unwrap(),
                            qty: Decimal::from_f64(96.84260000).unwrap(),
                        },
                    ],
                    asks: vec![
                        OrderBookUnit {
                            price: Decimal::from_f64(0.02359000).unwrap(),
                            qty: Decimal::from_f64(25.63400000).unwrap(),
                        },
                        OrderBookUnit {
                            price: Decimal::from_f64(0.02360000).unwrap(),
                            qty: Decimal::from_f64(53.22680000).unwrap(),
                        },
                        OrderBookUnit {
                            price: Decimal::from_f64(0.02361000).unwrap(),
                            qty: Decimal::from_f64(81.91300000).unwrap(),
                        },
                    ],
                },
                symbol_filter: SymbolFilter {
                    lot_size_step: 4,
                    tick_size: 5,
                    lot_size_min_qty: Decimal::from_f64(0.00010000).unwrap(),
                },
            },
        ];

        let orders = order_builder.calculate_chain_profit(&order_symbols);

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

    // Case: skipped, does not pass the minimum quantity.
    #[tokio::test]
    async fn test_calculate_chain_profit_5() -> anyhow::Result<()> {
        let market_api = match Binance::new(binance_api::Config::default()) {
            Ok(v) => v,
            Err(e) => bail!("Failed init binance client: {e}"),
        };

        let order_builder = OrderBuilder::new(market_api, 3);

        let order_symbols = vec![
            OrderSymbol {
                symbol: "ETHBTC".to_string(),
                base_asset: "ETH".to_string(),
                base_asset_precision: 8,
                quote_asset: "BTC".to_string(),
                quote_precision: 8,
                symbol_order: SymbolOrder::Asc,
                min_profit_qty: Decimal::from_f64(0.0),
                max_order_qty: Decimal::from_f64(0.0079),
                order_book: OrderBook {
                    last_update_id: 1,
                    bids: vec![OrderBookUnit {
                        price: Decimal::from_f64(0.03615000).unwrap(),
                        qty: Decimal::from_f64(0.20000000).unwrap(),
                    }],
                    asks: vec![OrderBookUnit {
                        price: Decimal::from_f64(0.03216000).unwrap(),
                        qty: Decimal::from_f64(2.22969000).unwrap(),
                    }],
                },
                symbol_filter: SymbolFilter {
                    lot_size_step: 4,
                    tick_size: 5,
                    lot_size_min_qty: Decimal::from_f64(0.00010000).unwrap(),
                },
            },
            OrderSymbol {
                symbol: "WBTCBTC".to_string(),
                base_asset: "WBTC".to_string(),
                base_asset_precision: 8,
                quote_asset: "BTC".to_string(),
                quote_precision: 8,
                symbol_order: SymbolOrder::Desc,
                min_profit_qty: None,
                max_order_qty: None,
                order_book: OrderBook {
                    last_update_id: 1,
                    bids: vec![OrderBookUnit {
                        price: Decimal::from_f64(0.99920000).unwrap(),
                        qty: Decimal::from_f64(19.28810000).unwrap(),
                    }],
                    asks: vec![OrderBookUnit {
                        price: Decimal::from_f64(0.99930000).unwrap(),
                        qty: Decimal::from_f64(0.9).unwrap(),
                    }],
                },
                symbol_filter: SymbolFilter {
                    lot_size_step: 5,
                    tick_size: 4,
                    lot_size_min_qty: Decimal::from_f64(0.00010000).unwrap(),
                },
            },
            OrderSymbol {
                symbol: "WBTCETH".to_string(),
                base_asset: "WBTC".to_string(),
                base_asset_precision: 8,
                quote_asset: "ETH".to_string(),
                quote_precision: 8,
                symbol_order: SymbolOrder::Asc,
                min_profit_qty: None,
                max_order_qty: None,
                order_book: OrderBook {
                    last_update_id: 1,
                    bids: vec![OrderBookUnit {
                        price: Decimal::from_f64(31.07000000).unwrap(),
                        qty: Decimal::from_f64(1.5).unwrap(), // <---- here
                    }],
                    asks: vec![OrderBookUnit {
                        price: Decimal::from_f64(31.08000000).unwrap(),
                        qty: Decimal::from_f64(25.63400000).unwrap(),
                    }],
                },
                symbol_filter: SymbolFilter {
                    lot_size_step: 5,
                    tick_size: 2,
                    lot_size_min_qty: Decimal::from_f64(0.00100000).unwrap(),
                },
            },
        ];

        let orders = order_builder.calculate_chain_profit(&order_symbols);
        assert_eq!(orders.len(), 0);

        Ok(())
    }

    #[tokio::test]
    async fn test_calculate_chain_profit_6() -> anyhow::Result<()> {
        let market_api = match Binance::new(binance_api::Config::default()) {
            Ok(v) => v,
            Err(e) => bail!("Failed init binance client: {e}"),
        };

        let order_builder = OrderBuilder::new(market_api, 1);

        let order_symbols = vec![
            OrderSymbol {
                symbol: "ETHBTC".to_string(),
                base_asset: "ETH".to_string(),
                base_asset_precision: 8,
                quote_asset: "BTC".to_string(),
                quote_precision: 8,
                symbol_order: SymbolOrder::Asc,
                min_profit_qty: Decimal::from_f64(0.0),
                max_order_qty: Decimal::from_f64(0.0079),
                order_book: OrderBook {
                    last_update_id: 1,
                    bids: vec![OrderBookUnit {
                        price: Decimal::from_f64(0.03402000).unwrap(),
                        qty: Decimal::from_f64(23.09700000).unwrap(),
                    }],
                    asks: vec![OrderBookUnit {
                        price: Decimal::from_f64(0.03203000).unwrap(),
                        qty: Decimal::from_f64(23.09700000).unwrap(),
                    }],
                },
                symbol_filter: SymbolFilter {
                    lot_size_step: 4,
                    tick_size: 5,
                    lot_size_min_qty: Decimal::from_f64(0.00010000).unwrap(),
                },
            },
            OrderSymbol {
                symbol: "SSVBTC".to_string(),
                base_asset: "SSV".to_string(),
                base_asset_precision: 8,
                quote_asset: "BTC".to_string(),
                quote_precision: 8,
                symbol_order: SymbolOrder::Desc,
                min_profit_qty: None,
                max_order_qty: None,
                order_book: OrderBook {
                    last_update_id: 1,
                    bids: vec![OrderBookUnit {
                        price: Decimal::from_f64(0.00007820).unwrap(),
                        qty: Decimal::from_f64(1.62000000).unwrap(),
                    }],
                    asks: vec![OrderBookUnit {
                        price: Decimal::from_f64(0.00007810).unwrap(),
                        qty: Decimal::from_f64(1.62000000).unwrap(),
                    }],
                },
                symbol_filter: SymbolFilter {
                    lot_size_step: 2,
                    tick_size: 7,
                    lot_size_min_qty: Decimal::from_f64(0.00010000).unwrap(),
                },
            },
            OrderSymbol {
                symbol: "SSVETH".to_string(),
                base_asset: "SSV".to_string(),
                base_asset_precision: 8,
                quote_asset: "ETH".to_string(),
                quote_precision: 8,
                symbol_order: SymbolOrder::Asc,
                min_profit_qty: None,
                max_order_qty: None,
                order_book: OrderBook {
                    last_update_id: 1,
                    bids: vec![OrderBookUnit {
                        price: Decimal::from_f64(0.00243200).unwrap(),
                        qty: Decimal::from_f64(0.54000000).unwrap(), // <---- here
                    }],
                    asks: vec![OrderBookUnit {
                        price: Decimal::from_f64(0.00243300).unwrap(),
                        qty: Decimal::from_f64(0.54000000).unwrap(),
                    }],
                },
                symbol_filter: SymbolFilter {
                    lot_size_step: 2,
                    tick_size: 6,
                    lot_size_min_qty: Decimal::from_f64(0.00100000).unwrap(),
                },
            },
        ];

        let orders = order_builder.calculate_chain_profit(&order_symbols);

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
