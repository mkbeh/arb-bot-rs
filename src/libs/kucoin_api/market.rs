use crate::libs::kucoin_api::{
    api::{Api, Spot},
    client::Client,
    enums::MarketType,
    models::{AllTickers, RestResponse, Symbol},
};

#[derive(Clone)]
pub struct Market {
    pub client: Client,
}

impl Market {
    pub async fn get_all_symbols(
        &self,
        market: Option<MarketType>,
    ) -> anyhow::Result<RestResponse<Vec<Symbol>>> {
        let mut params: Vec<(&str, &str)> = vec![];

        if let Some(market) = market {
            params.push(("market", market.as_str()))
        };

        self.client
            .get(Api::Spot(Spot::GetAllSymbols), Some(&params))
            .await
    }

    pub async fn get_all_tickers(&self) -> anyhow::Result<RestResponse<AllTickers>> {
        self.client.get(Api::Spot(Spot::GetAllTickers), None).await
    }
}
