use std::time::SystemTime;

use crate::libs::binance_api::{
    AccountInformation,
    api::{Api, Spot},
    client::Client,
    utils,
};

pub struct Account {
    pub client: Client,
}

impl Account {
    pub async fn get_account<S>(&self, recv_window: S) -> anyhow::Result<AccountInformation>
    where
        S: Into<String>,
    {
        let recv_window = recv_window.into();
        let ts = (utils::get_timestamp(SystemTime::now())?).to_string();

        let params = &vec![
            ("omitZeroBalances", "true"),
            ("recvWindow", recv_window.as_str()),
            ("timestamp", ts.as_str()),
        ];

        self.client
            .get(Api::Spot(Spot::Account), Some(params), true)
            .await
    }
}
