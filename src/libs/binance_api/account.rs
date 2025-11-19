use std::time::SystemTime;

use crate::libs::binance_api::{
    AccountInformation,
    api::{Api, Spot},
    client::Client,
    utils,
};

/// Wrapper for managing Binance account operations via the API client.
#[derive(Clone)]
pub struct Account {
    pub client: Client,
}

impl Account {
    /// Fetches the current account information from the Binance API.
    pub async fn get_account<S, T>(
        &self,
        omit_zero_balances: S,
        recv_window: T,
    ) -> anyhow::Result<AccountInformation>
    where
        S: ToString,
        T: ToString,
    {
        let recv_window = recv_window.to_string();
        let ts = utils::get_timestamp(SystemTime::now())?;

        let params: Vec<(String, String)> = vec![
            (
                "omitZeroBalances".to_owned(),
                omit_zero_balances.to_string(),
            ),
            ("recvWindow".to_owned(), recv_window),
            ("timestamp".to_owned(), ts.to_string()),
        ];

        self.client
            .get(Api::Spot(Spot::Account), Some(&params), true)
            .await
    }
}
