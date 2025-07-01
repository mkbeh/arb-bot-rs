use crate::libs::binance_api::{
    OrderBook,
    api::{Api, Spot},
    client::Client,
};

#[derive(Clone)]
pub struct Market {
    pub client: Client,
}

impl Market {
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
}
