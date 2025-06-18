use crate::libs::binance_api::{
    api::{Api, Spot},
    client::Client,
    models::ExchangeInformation,
};

pub struct General {
    pub(crate) client: Client,
}

impl General {
    /// Exchange information.
    pub async fn exchange_info(&self) -> anyhow::Result<ExchangeInformation> {
        let params = vec![
            ("symbolStatus", "TRADING"),
            ("showPermissionSets", "false"),
            ("permissions", "[\"SPOT\"]"),
        ];

        self.client
            .get(Api::Spot(Spot::ExchangeInfo), None, &params)
            .await
    }
}
