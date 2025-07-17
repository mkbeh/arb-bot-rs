use std::{
    ops::Sub,
    time::Duration,
};

use anyhow::bail;
use rust_decimal::{Decimal, prelude::Zero};
use tracing::info_span;

use crate::{
    config::Asset,
    libs::binance_api::{Market, OrderBook, OrderBookUnit},
    services::{
        binance::{ChainSymbol, REQUEST_WEIGHT},
        enums::SymbolOrder,
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
    pub min_profit_limit: Option<Decimal>,
    pub max_volume_limit: Option<Decimal>,
    pub order_book: OrderBook,
}

#[derive(Clone, Debug)]
struct Order {
    symbol: String,
    symbol_order: SymbolOrder,
    price: Decimal,
    base_qty: Decimal,
    base_precision: u32,
    quote_qty: Decimal,
    quote_precision: u32,
    order_book_units: Vec<OrderBookUnit>,
}

pub struct OrderBuilder {
    base_assets: Vec<Asset>,
    market_api: Market,
    market_depth_limit: usize,
}

impl OrderBuilder {
    pub fn new(base_assets: Vec<Asset>, market_api: Market, market_depth_limit: usize) -> Self {
        Self {
            base_assets,
            market_api,
            market_depth_limit,
        }
    }

    pub async fn build_chains_orders(&self, chains: Vec<[ChainSymbol; 3]>) -> anyhow::Result<()> {
        let find_base_asset_fn = |chain_symbol: &ChainSymbol| -> Option<&Asset> {
            self.base_assets.iter().find(|&x| {
                if chain_symbol.order == SymbolOrder::Asc {
                    x.asset == chain_symbol.symbol.base_asset
                } else {
                    x.asset == chain_symbol.symbol.quote_asset
                }
            })
        };

        for chain in chains.iter() {
            let mut request_weight = REQUEST_WEIGHT.lock().await;

            // Calculate request weight, where api method 'get depth' cost 5 weight and api method
            // 'send order' cost 1 weight - need x3 requests for each symbol.
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

            // todo: desc
            let mut order_symbols = vec![];

            for (i, chain_symbol) in chain.iter().enumerate() {
                let mut min_profit_limit = None;
                let mut max_volume_limit = None;

                // define limits for 1st pair
                if i == 0 {
                    let base_asset = match find_base_asset_fn(chain_symbol) {
                        Some(base) => base,
                        _ => bail!(
                            "failed to find base asset for symbol {}",
                            chain_symbol.symbol.symbol
                        ),
                    };

                    min_profit_limit = Some(base_asset.min_profit_limit);
                    max_volume_limit = Some(base_asset.max_volume_limit);
                }

                let s = &chain_symbol.symbol;
                order_symbols.push(OrderSymbol {
                    symbol: s.symbol.clone(),
                    base_asset: s.base_asset.clone(),
                    base_asset_precision: s.base_asset_precision,
                    quote_asset: s.quote_asset.clone(),
                    quote_precision: s.quote_precision,
                    symbol_order: chain_symbol.order,
                    min_profit_limit,
                    max_volume_limit,
                    order_book: order_books[i].clone(),
                });
            }

            let orders = self.calculate_chain_profit(&order_symbols);
            if orders.is_empty() {
                request_weight.sub_weight(3);
                continue;
            }

            info_span!("received profit", orders=?orders, chain = ?chain);

            // todo: send orders
        }

        info_span!("all chain have been completed", chains = chains.len());

        Ok(())
    }

    fn calculate_chain_profit(&self, order_symbols: &[OrderSymbol]) -> Vec<Order> {
        // Recalculate volumes for all pairs except 1st.
        let recalc_orders_qty_fn = |orders: &mut Vec<Order>, order: usize| {
            if order == 0 {
                return;
            }

            let orders_count = orders.len();
            let mut count = 1;

            while count <= order {
                let order_a_idx = orders_count - count - 1;
                let order_b_idx = orders_count - count;

                let order_a = &orders[order_a_idx];
                let order_b = &orders[order_b_idx];

                if order_a.quote_qty == order_b.base_qty {
                    // unexpected logic
                    return;
                }

                let precision = match order_a.symbol_order {
                    SymbolOrder::Asc => order_a.base_precision,
                    SymbolOrder::Desc => order_a.quote_precision,
                };
                let base_qty = match order_a.symbol_order {
                    SymbolOrder::Asc => (order_b.base_qty / order_a.price).round_dp(precision),
                    SymbolOrder::Desc => (order_b.base_qty * order_a.price).round_dp(precision),
                };

                {
                    orders[order_a_idx].quote_qty = order_b.base_qty;
                    orders[order_a_idx].base_qty = base_qty;
                }

                count += 1;
            }
        };

        let define_precision = |order_symbol: &OrderSymbol| -> u32 {
            match order_symbol.symbol_order {
                SymbolOrder::Asc => order_symbol.base_asset_precision,
                SymbolOrder::Desc => order_symbol.quote_precision,
            }
        };

        let mut orders: Vec<Order> = vec![];
        let mut depth_limit = 0;

        while depth_limit < self.market_depth_limit {
            for (i, order_symbol) in order_symbols.iter().enumerate() {
                let order_units = match order_symbol.symbol_order {
                    SymbolOrder::Asc => order_symbol.order_book.bids.clone(),
                    SymbolOrder::Desc => order_symbol.order_book.asks.clone(),
                };

                let base_qty_limit = if i == 0 {
                    order_symbol.max_volume_limit.expect("unexpected logic")
                } else {
                    orders[orders.len() - 1].quote_qty
                };

                let mut base_qty = Decimal::zero();
                let mut price = Decimal::zero();
                let mut order_book_units = vec![];

                for order_unit in order_units.iter().take(depth_limit + 1) {
                    let qty = match order_symbol.symbol_order {
                        SymbolOrder::Asc => order_unit.qty,
                        SymbolOrder::Desc => order_unit.qty * order_unit.price,
                    };

                    base_qty = (base_qty + qty).round_dp(define_precision(order_symbol));

                    price = order_unit.price;
                    order_book_units.push(order_unit.clone());

                    if base_qty >= base_qty_limit {
                        base_qty = base_qty_limit;
                        break;
                    }
                }

                let quote_qty = match order_symbol.symbol_order {
                    SymbolOrder::Asc => {
                        (base_qty * price).round_dp(order_symbol.base_asset_precision)
                    }

                    SymbolOrder::Desc => (base_qty / price).round_dp(order_symbol.quote_precision),
                };

                let order = Order {
                    symbol: order_symbol.symbol.clone(),
                    symbol_order: order_symbol.symbol_order,
                    price,
                    base_qty,
                    base_precision: order_symbol.base_asset_precision,
                    quote_qty,
                    quote_precision: order_symbol.quote_precision,
                    order_book_units,
                };

                orders.push(order);

                // If it is not the first symbol and the base quantity does not match the limit qty,
                // then it is necessary to recalculate the volume of previous orders
                if i != 0 && base_qty < base_qty_limit {
                    recalc_orders_qty_fn(&mut orders, i);
                }
            }

            // Compare first chain order and first chain item volume limit.
            // If they are equal, there is no point in trying to sum up the volumes, so break.
            if orders[orders.len() - order_symbols.len()].base_qty
                == order_symbols[0].max_volume_limit.unwrap()
            {
                break;
            }

            depth_limit += 1;
        }

        let mut profit_orders = vec![];
        let mut last_profit = Decimal::zero();

        // Iterate over every first order in chain.
        for i in (0..).take(orders.len() - 1).step_by(order_symbols.len()) {
            // Difference between the outbound volume of the last chain and the inbound volume of
            // the first chain.
            let diff_qty = orders[i + 2].quote_qty - orders[i].base_qty;

            if diff_qty >= order_symbols.first().unwrap().min_profit_limit.unwrap()
                && diff_qty > last_profit
            {
                last_profit = diff_qty;
                profit_orders.extend_from_slice(&orders[i..=i + 2]);
            }
        }

        // Return 3 last profit orders
        if profit_orders.len() >= order_symbols.len() {
            let idx = profit_orders.len().sub(order_symbols.len());
            profit_orders[idx..].to_vec()
        } else {
            profit_orders
        }
    }
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
                    status: "TRADING".to_owned(),
                    base_asset: "BTC".to_owned(),
                    base_asset_precision: 8,
                    quote_asset: "USDT".to_owned(),
                    quote_precision: 8,
                    ..Default::default()
                },
                order: SymbolOrder::Asc,
            },
            ChainSymbol {
                symbol: Symbol {
                    symbol: "ETHUSDT".to_owned(),
                    status: "TRADING".to_owned(),
                    base_asset: "ETH".to_owned(),
                    base_asset_precision: 8,
                    quote_asset: "USDT".to_owned(),
                    quote_precision: 8,
                    ..Default::default()
                },
                order: SymbolOrder::Desc,
            },
            ChainSymbol {
                symbol: Symbol {
                    symbol: "ETHBTC".to_owned(),
                    status: "TRADING".to_owned(),
                    base_asset: "ETH".to_owned(),
                    base_asset_precision: 8,
                    quote_asset: "BTC".to_owned(),
                    quote_precision: 8,
                    ..Default::default()
                },
                order: SymbolOrder::Asc,
            },
        ]];

        let base_assets: Vec<Asset> = vec![
            Asset {
                asset: "BTC".to_string(),
                asset_precision: 8,
                symbol: Some("BTCUSDT".to_owned()),
                min_profit_limit: Decimal::from_f64(0.000030).unwrap(),
                max_volume_limit: Decimal::from_f64(0.00030).unwrap(),
            },
            Asset {
                asset: "ETH".to_string(),
                asset_precision: 8,
                symbol: Some("ETHUSDT".to_owned()),
                min_profit_limit: Decimal::from_f64(0.0012).unwrap(),
                max_volume_limit: Decimal::from_f64(0.012).unwrap(),
            },
            Asset {
                asset: "USDT".to_string(),
                asset_precision: 8,
                symbol: Some("USDT".to_owned()),
                min_profit_limit: Decimal::from_f64(3.0).unwrap(),
                max_volume_limit: Decimal::from_f64(30.0).unwrap(),
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

        let orders_builder = OrderBuilder::new(base_assets, market_api, 5);
        let chains_orders = orders_builder.build_chains_orders(test_chains).await;

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

        let order_builder = OrderBuilder::new(vec![], market_api, 3);

        let order_symbols = vec![
            OrderSymbol {
                symbol: "BTCUSDT".to_string(),
                base_asset: "BTC".to_string(),
                base_asset_precision: 8,
                quote_asset: "USDT".to_string(),
                quote_precision: 8,
                symbol_order: SymbolOrder::Asc,
                min_profit_limit: Decimal::from_f64(0.000030),
                max_volume_limit: Decimal::from_f64(0.00030),
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
            },
            OrderSymbol {
                symbol: "ETHUSDT".to_string(),
                base_asset: "ETH".to_string(),
                base_asset_precision: 8,
                quote_asset: "USDT".to_string(),
                quote_precision: 8,
                symbol_order: SymbolOrder::Desc,
                min_profit_limit: None,
                max_volume_limit: None,
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
            },
            OrderSymbol {
                symbol: "ETHBTC".to_string(),
                base_asset: "ETH".to_string(),
                base_asset_precision: 8,
                quote_asset: "BTC".to_string(),
                quote_precision: 8,
                symbol_order: SymbolOrder::Asc,
                min_profit_limit: None,
                max_volume_limit: None,
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
            },
        ];

        let orders = order_builder.calculate_chain_profit(&order_symbols);

        assert_eq!(orders.len(), 3);

        assert_eq!(orders[0].symbol, "BTCUSDT", "symbol={}", 0);
        assert_eq!(orders[0].symbol_order, SymbolOrder::Asc, "order={}", 0);
        assert_eq!(orders[0].price.to_string(), "109615.46", "price={}", 0);
        assert_eq!(orders[0].base_qty.to_string(), "0.0003", "b_qty={}", 0);
        assert_eq!(orders[0].quote_qty.to_string(), "32.884638", "q_qty={}", 0);

        assert_eq!(orders[1].symbol, "ETHUSDT", "symbol={}", 1);
        assert_eq!(orders[1].symbol_order, SymbolOrder::Desc, "order={}", 1);
        assert_eq!(orders[1].price.to_string(), "2585.71", "price={}", 1);
        assert_eq!(orders[1].base_qty.to_string(), "32.884638", "b_qty={}", 1);
        assert_eq!(orders[1].quote_qty.to_string(), "0.01271784", "q_qty={}", 1);

        assert_eq!(orders[2].symbol, "ETHBTC", "symbol={}", 2);
        assert_eq!(orders[2].symbol_order, SymbolOrder::Asc, "order={}", 2);
        assert_eq!(orders[2].price.to_string(), "0.02858", "price={}", 2);
        assert_eq!(orders[2].base_qty.to_string(), "0.01271784", "b_qty={}", 2);
        assert_eq!(orders[2].quote_qty.to_string(), "0.00036348", "q_qty={}", 2);

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

        let order_builder = OrderBuilder::new(vec![], market_api, 3);

        let order_symbols = vec![
            OrderSymbol {
                symbol: "BTCUSDT".to_string(),
                base_asset: "BTC".to_string(),
                base_asset_precision: 8,
                quote_asset: "USDT".to_string(),
                quote_precision: 8,
                symbol_order: SymbolOrder::Asc,
                min_profit_limit: Decimal::from_f64(0.000030),
                max_volume_limit: Decimal::from_f64(0.00030),
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
            },
            OrderSymbol {
                symbol: "ETHUSDT".to_string(),
                base_asset: "ETH".to_string(),
                base_asset_precision: 8,
                quote_asset: "USDT".to_string(),
                quote_precision: 8,
                symbol_order: SymbolOrder::Desc,
                min_profit_limit: None,
                max_volume_limit: None,
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
            },
            OrderSymbol {
                symbol: "ETHBTC".to_string(),
                base_asset: "ETH".to_string(),
                base_asset_precision: 8,
                quote_asset: "BTC".to_string(),
                quote_precision: 8,
                symbol_order: SymbolOrder::Asc,
                min_profit_limit: None,
                max_volume_limit: None,
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
            },
        ];

        let orders = order_builder.calculate_chain_profit(&order_symbols);

        assert_eq!(orders.len(), 3);

        assert_eq!(orders[0].symbol, "BTCUSDT", "symbol={}", 0);
        assert_eq!(orders[0].symbol_order, SymbolOrder::Asc, "order={}", 0);
        assert_eq!(orders[0].price.to_string(), "109614.96", "price={}", 0);
        assert_eq!(orders[0].base_qty.to_string(), "0.0003", "b_qty={}", 0);
        assert_eq!(orders[0].quote_qty.to_string(), "32.884488", "q_qty={}", 0);

        assert_eq!(orders[1].symbol, "ETHUSDT", "symbol={}", 1);
        assert_eq!(orders[1].symbol_order, SymbolOrder::Desc, "order={}", 1);
        assert_eq!(orders[1].price.to_string(), "2585.71", "price={}", 1);
        assert_eq!(orders[1].base_qty.to_string(), "32.884488", "b_qty={}", 1);
        assert_eq!(orders[1].quote_qty.to_string(), "0.01271778", "q_qty={}", 1);

        assert_eq!(orders[2].symbol, "ETHBTC", "symbol={}", 2);
        assert_eq!(orders[2].symbol_order, SymbolOrder::Asc, "order={}", 2);
        assert_eq!(orders[2].price.to_string(), "0.02858", "price={}", 2);
        assert_eq!(orders[2].base_qty.to_string(), "0.01271778", "b_qty={}", 2);
        assert_eq!(orders[2].quote_qty.to_string(), "0.00036347", "q_qty={}", 2);

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

        let order_builder = OrderBuilder::new(vec![], market_api, 3);

        let order_symbols = vec![
            OrderSymbol {
                symbol: "BTCUSDT".to_string(),
                base_asset: "BTC".to_string(),
                base_asset_precision: 8,
                quote_asset: "USDT".to_string(),
                quote_precision: 8,
                symbol_order: SymbolOrder::Asc,
                min_profit_limit: Decimal::from_f64(0.000030),
                max_volume_limit: Decimal::from_f64(0.00030),
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
            },
            OrderSymbol {
                symbol: "ETHUSDT".to_string(),
                base_asset: "ETH".to_string(),
                base_asset_precision: 8,
                quote_asset: "USDT".to_string(),
                quote_precision: 8,
                symbol_order: SymbolOrder::Desc,
                min_profit_limit: None,
                max_volume_limit: None,
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
            },
            OrderSymbol {
                symbol: "ETHBTC".to_string(),
                base_asset: "ETH".to_string(),
                base_asset_precision: 8,
                quote_asset: "BTC".to_string(),
                quote_precision: 8,
                symbol_order: SymbolOrder::Asc,
                min_profit_limit: None,
                max_volume_limit: None,
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
            },
        ];

        let orders = order_builder.calculate_chain_profit(&order_symbols);

        assert_eq!(orders.len(), 3);

        assert_eq!(orders[0].symbol, "BTCUSDT", "symbol={}", 0);
        assert_eq!(orders[0].symbol_order, SymbolOrder::Asc, "order={}", 0);
        assert_eq!(orders[0].price.to_string(), "109615.46", "price={}", 0);
        assert_eq!(orders[0].base_qty.to_string(), "0.0003", "b_qty={}", 0);
        assert_eq!(orders[0].quote_qty.to_string(), "32.884638", "q_qty={}", 0);

        assert_eq!(orders[1].symbol, "ETHUSDT", "symbol={}", 1);
        assert_eq!(orders[1].symbol_order, SymbolOrder::Desc, "order={}", 1);
        assert_eq!(orders[1].price.to_string(), "2585.72", "price={}", 1);
        assert_eq!(orders[1].base_qty.to_string(), "32.884638", "b_qty={}", 1);
        assert_eq!(orders[1].quote_qty.to_string(), "0.01271779", "q_qty={}", 1);

        assert_eq!(orders[2].symbol, "ETHBTC", "symbol={}", 2);
        assert_eq!(orders[2].symbol_order, SymbolOrder::Asc, "order={}", 2);
        assert_eq!(orders[2].price.to_string(), "0.02858", "price={}", 2);
        assert_eq!(orders[2].base_qty.to_string(), "0.01271779", "b_qty={}", 2);
        assert_eq!(orders[2].quote_qty.to_string(), "0.00036347", "q_qty={}", 2);

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

        let order_builder = OrderBuilder::new(vec![], market_api, 3);

        let order_symbols = vec![
            OrderSymbol {
                symbol: "BTCUSDT".to_string(),
                base_asset: "BTC".to_string(),
                base_asset_precision: 8,
                quote_asset: "USDT".to_string(),
                quote_precision: 8,
                symbol_order: SymbolOrder::Asc,
                min_profit_limit: Decimal::from_f64(0.000030),
                max_volume_limit: Decimal::from_f64(0.00030),
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
            },
            OrderSymbol {
                symbol: "ETHUSDT".to_string(),
                base_asset: "ETH".to_string(),
                base_asset_precision: 8,
                quote_asset: "USDT".to_string(),
                quote_precision: 8,
                symbol_order: SymbolOrder::Desc,
                min_profit_limit: None,
                max_volume_limit: None,
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
            },
            OrderSymbol {
                symbol: "ETHBTC".to_string(),
                base_asset: "ETH".to_string(),
                base_asset_precision: 8,
                quote_asset: "BTC".to_string(),
                quote_precision: 8,
                symbol_order: SymbolOrder::Asc,
                min_profit_limit: None,
                max_volume_limit: None,
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
            },
        ];

        let orders = order_builder.calculate_chain_profit(&order_symbols);

        assert_eq!(orders.len(), 3);

        assert_eq!(orders[0].symbol, "BTCUSDT", "symbol={}", 0);
        assert_eq!(orders[0].symbol_order, SymbolOrder::Asc, "order={}", 0);
        assert_eq!(orders[0].price.to_string(), "109615.46", "price={}", 0);
        assert_eq!(orders[0].base_qty.to_string(), "0.0003", "b_qty={}", 0);
        assert_eq!(orders[0].quote_qty.to_string(), "32.884638", "q_qty={}", 0);

        assert_eq!(orders[1].symbol, "ETHUSDT", "symbol={}", 1);
        assert_eq!(orders[1].symbol_order, SymbolOrder::Desc, "order={}", 1);
        assert_eq!(orders[1].price.to_string(), "2585.71", "price={}", 1);
        assert_eq!(orders[1].base_qty.to_string(), "32.884638", "b_qty={}", 1);
        assert_eq!(orders[1].quote_qty.to_string(), "0.01271784", "q_qty={}", 1);

        assert_eq!(orders[2].symbol, "ETHBTC", "symbol={}", 2);
        assert_eq!(orders[2].symbol_order, SymbolOrder::Asc, "order={}", 2);
        assert_eq!(orders[2].price.to_string(), "0.02857", "price={}", 2);
        assert_eq!(orders[2].base_qty.to_string(), "0.01271784", "b_qty={}", 2);
        assert_eq!(orders[2].quote_qty.to_string(), "0.00036335", "q_qty={}", 2);

        Ok(())
    }
}
