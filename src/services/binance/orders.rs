use anyhow::bail;

use crate::{
    libs::binance_api::{Market, OrderBook},
    services::binance::SymbolWrapper,
};

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

    pub async fn build_chains_orders(self, chains: Vec<[SymbolWrapper; 3]>) -> anyhow::Result<()> {
        for chain in chains {
            let tasks: Vec<_> = chain
                .into_iter()
                .map(|wrapper| {
                    let client = self.market_api.clone();
                    tokio::spawn(async move {
                        client
                            .get_depth(wrapper.symbol.symbol.clone(), &self.market_depth_limit)
                            .await
                    })
                })
                .collect();

            let mut order_books: Vec<OrderBook> = vec![];
            for task in tasks {
                match task.await {
                    Ok(result) => match result {
                        Ok(order_book) => order_books.push(order_book),
                        Err(e) => bail!(e),
                    },
                    Err(e) => bail!(e),
                }
            }

            println!("order_books: {order_books:?}");
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

        let payload_ethbtc = r#"
        {
           "lastUpdateId":8213849747,
           "bids":[
              [
                 "0.02372000",
                 "38.51100000"
              ],
              [
                 "0.02371000",
                 "87.93970000"
              ],
              [
                 "0.02370000",
                 "78.48140000"
              ],
              [
                 "0.02369000",
                 "66.43220000"
              ],
              [
                 "0.02368000",
                 "106.54400000"
              ]
           ],
           "asks":[
              [
                 "0.02373000",
                 "52.62810000"
              ],
              [
                 "0.02374000",
                 "105.25140000"
              ],
              [
                 "0.02375000",
                 "95.67120000"
              ],
              [
                 "0.02376000",
                 "85.33240000"
              ],
              [
                 "0.02377000",
                 "64.57160000"
              ]
           ]
        }
    "#;

        let payload_ltcbtc = r#"
        {
           "lastUpdateId":8213849747,
           "bids":[
              [
                 "0.02372000",
                 "38.51100000"
              ],
              [
                 "0.02371000",
                 "87.93970000"
              ],
              [
                 "0.02370000",
                 "78.48140000"
              ],
              [
                 "0.02369000",
                 "66.43220000"
              ],
              [
                 "0.02368000",
                 "106.54400000"
              ]
           ],
           "asks":[
              [
                 "0.02373000",
                 "52.62810000"
              ],
              [
                 "0.02374000",
                 "105.25140000"
              ],
              [
                 "0.02375000",
                 "95.67120000"
              ],
              [
                 "0.02376000",
                 "85.33240000"
              ],
              [
                 "0.02377000",
                 "64.57160000"
              ]
           ]
        }
    "#;

        let payload_ltceth = r#"
        {
           "lastUpdateId":8213849747,
           "bids":[
              [
                 "0.02372000",
                 "38.51100000"
              ],
              [
                 "0.02371000",
                 "87.93970000"
              ],
              [
                 "0.02370000",
                 "78.48140000"
              ],
              [
                 "0.02369000",
                 "66.43220000"
              ],
              [
                 "0.02368000",
                 "106.54400000"
              ]
           ],
           "asks":[
              [
                 "0.02373000",
                 "52.62810000"
              ],
              [
                 "0.02374000",
                 "105.25140000"
              ],
              [
                 "0.02375000",
                 "95.67120000"
              ],
              [
                 "0.02376000",
                 "85.33240000"
              ],
              [
                 "0.02377000",
                 "64.57160000"
              ]
           ]
        }
    "#;

        let mock_order_book_ethbtc = server
            .mock("GET", "/api/v3/depth")
            .with_header("content-type", "application/json;charset=UTF-8")
            .match_query(Matcher::Regex("symbol=ETHBTC&limit=5".into()))
            .with_body(payload_ethbtc)
            .create_async();

        let mock_order_book_ltcbtc = server
            .mock("GET", "/api/v3/depth")
            .with_header("content-type", "application/json;charset=UTF-8")
            .match_query(Matcher::Regex("symbol=LTCBTC&limit=5".into()))
            .with_body(payload_ltcbtc)
            .create_async();

        let mock_order_book_ltceth = server
            .mock("GET", "/api/v3/depth")
            .with_header("content-type", "application/json;charset=UTF-8")
            .match_query(Matcher::Regex("symbol=LTCETH&limit=5".into()))
            .with_body(payload_ltceth)
            .create_async();

        let (mock_order_book_ethbtc, mock_order_book_ltcbtc, mock_order_book_ltceth) = futures::join!(
            mock_order_book_ethbtc,
            mock_order_book_ltcbtc,
            mock_order_book_ltceth
        );

        let test_chains = vec![[
            SymbolWrapper {
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
            SymbolWrapper {
                symbol: Symbol {
                    symbol: "LTCBTC".to_owned(),
                    status: "TRADING".to_owned(),
                    base_asset: "LTC".to_owned(),
                    base_asset_precision: 8,
                    quote_asset: "BTC".to_owned(),
                    quote_precision: 8,
                    ..Default::default()
                },
                order: SymbolOrder::Desc,
            },
            SymbolWrapper {
                symbol: Symbol {
                    symbol: "LTCETH".to_owned(),
                    status: "TRADING".to_owned(),
                    base_asset: "LTC".to_owned(),
                    base_asset_precision: 8,
                    quote_asset: "ETH".to_owned(),
                    quote_precision: 8,
                    ..Default::default()
                },
                order: SymbolOrder::Asc,
            },
        ]];

        let api_config = binance_api::Config {
            host: server.url(),
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
