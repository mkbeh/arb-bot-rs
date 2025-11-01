use crate::libs::kucoin_api::{ClientConfig, Market, client::Client};

pub enum Api {
    Spot(Spot),
}

pub enum Spot {
    GetAllSymbols,
}

impl From<Api> for String {
    fn from(item: Api) -> Self {
        String::from(match item {
            Api::Spot(route) => match route {
                Spot::GetAllSymbols => "/api/v2/symbols",
            },
        })
    }
}

pub trait Kucoin {
    fn new(cfg: ClientConfig) -> anyhow::Result<Self>
    where
        Self: Sized;
}

impl Kucoin for Market {
    fn new(cfg: ClientConfig) -> anyhow::Result<Market> {
        Ok(Market {
            client: Client::from_config(cfg)?,
        })
    }
}
