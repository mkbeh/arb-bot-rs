use crate::libs::binance_client::{
    api::{Api, Spot},
    client::Client,
    models::ExchangeInformation,
};

#[derive(Clone)]
pub struct General {
    pub client: Client,
}

impl General {
    /// Exchange information.
    pub async fn exchange_info(&self) -> anyhow::Result<ExchangeInformation> {
        let params: Vec<(String, String)> = vec![
            ("symbolStatus".to_owned(), "TRADING".to_string()),
            ("showPermissionSets".to_owned(), "false".to_string()),
            ("permissions".to_owned(), "[\"SPOT\"]".to_string()),
        ];

        self.client
            .get(Api::Spot(Spot::ExchangeInfo), Some(&params), false)
            .await
    }
}
