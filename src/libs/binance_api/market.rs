use crate::libs::binance_api::{
    OrderBook, TickerPriceResponseType, TickerPriceStats,
    api::{Api, Spot},
    client::Client,
};

#[derive(Clone)]
pub struct Market {
    pub client: Client,
}

impl Market {
    // Order book.
    pub async fn get_depth<S, T>(&self, symbol: S, limit: T) -> anyhow::Result<OrderBook>
    where
        S: ToString,
        T: ToString,
    {
        let params: Vec<(String, String)> = vec![
            ("symbol".to_owned(), symbol.to_string()),
            ("limit".to_owned(), limit.to_string()),
        ];

        self.client
            .get(Api::Spot(Spot::Depth), Some(&params), false)
            .await
    }

    // 24hr ticker price change statistics.
    pub async fn get_ticker_price_24h<S>(
        &self,
        symbols: Option<Vec<S>>,
        response_type: TickerPriceResponseType,
    ) -> anyhow::Result<Vec<TickerPriceStats>>
    where
        S: ToString,
    {
        let mut params: Vec<(String, String)> =
            vec![("type".to_owned(), response_type.to_string())];

        if let Some(symbols) = symbols {
            params.push((
                "symbols".to_owned(),
                format!(
                    "[{}]",
                    symbols
                        .iter()
                        .map(|x| format!("\"{}\"", x.to_string()))
                        .collect::<Vec<_>>()
                        .join(",")
                ),
            ));
        }

        self.client
            .get(Api::Spot(Spot::Ticker24hr), Some(&params), false)
            .await
    }
}
