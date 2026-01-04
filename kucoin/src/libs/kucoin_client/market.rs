use crate::libs::kucoin_client::{
    api::{Api, Spot},
    client::Client,
    enums::MarketType,
    models::{AllTickers, RestResponse, Symbol},
};

/// Wrapper struct for market-related KuCoin API operations.
#[derive(Clone)]
pub struct Market {
    pub client: Client,
}

impl Market {
    /// Retrieves all trading symbols from KuCoin Spot API.
    pub async fn get_all_symbols(
        &self,
        market: Option<MarketType>,
    ) -> anyhow::Result<RestResponse<Vec<Symbol>>> {
        let mut params: Vec<(&str, &str)> = vec![];

        if let Some(market) = market {
            params.push(("market", market.as_str()))
        };

        self.client
            .get(Api::Spot(Spot::GetAllSymbols), Some(&params), false)
            .await
    }

    /// Retrieves all tickers (price data) for trading pairs on KuCoin Spot API.
    pub async fn get_all_tickers(&self) -> anyhow::Result<RestResponse<AllTickers>> {
        self.client
            .get(Api::Spot(Spot::GetAllTickers), None, false)
            .await
    }
}
