use anyhow::bail;

use crate::{
    libs::binance_api::{Market, OrderBook},
    services::{binance::ChainSymbol, enums::SymbolOrder},
};

#[derive(Clone, Debug)]
pub struct OrderSymbol {
    pub symbol: String,
    pub status: String,
    pub base_asset: String,
    pub base_asset_precision: u64,
    pub quote_asset: String,
    pub quote_precision: u64,
    pub base_commission_precision: u64,
    pub quote_commission_precision: u64,
    pub symbol_order: SymbolOrder,
    pub order_book: OrderBook,
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

    pub async fn build_chains_orders(&self, chains: Vec<[ChainSymbol; 3]>) -> anyhow::Result<()> {
        for chain in &chains {
            let mut order_symbols = vec![];

            for wrapper in chain {
                let order_book = match self
                    .market_api
                    .get_depth(wrapper.symbol.symbol.clone(), &self.market_depth_limit)
                    .await
                {
                    Ok(order_book) => order_book,
                    Err(e) => bail!("failed to get symbol order book: {}", e),
                };

                let s = &wrapper.symbol;
                order_symbols.push(OrderSymbol {
                    symbol: s.symbol.clone(),
                    status: s.status.clone(),
                    base_asset: s.base_asset.clone(),
                    base_asset_precision: s.base_asset_precision,
                    quote_asset: s.quote_asset.clone(),
                    quote_precision: s.quote_precision,
                    base_commission_precision: s.base_commission_precision,
                    quote_commission_precision: s.quote_commission_precision,
                    symbol_order: wrapper.order,
                    order_book,
                });
            }

            println!("{order_symbols:#?}");

            // todo: add calculation
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use mockito::{Matcher, Server};

    use super::*;
    use crate::{
        libs::{
            binance_api,
            binance_api::{Binance, Symbol},
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

        let api_config = binance_api::Config {
            api_url: server.url(),
            ..Default::default()
        };

        let market_api = match Binance::new(api_config.clone()) {
            Ok(v) => v,
            Err(e) => bail!("Failed init binance client: {e}"),
        };

        let orders_builder = OrderBuilder::new(market_api, 5);
        let chains_orders = orders_builder.build_chains_orders(test_chains).await;

        mock_order_book_ethbtc.assert_async().await;
        mock_order_book_ltcbtc.assert_async().await;
        mock_order_book_ltceth.assert_async().await;

        Ok(())
    }
}
