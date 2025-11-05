use crate::libs::kucoin_api::{BaseInfo, ClientConfig, Market, client::Client};

pub enum Api {
    Spot(Spot),
}

pub enum Spot {
    GetAllSymbols,
    GetAllTickers,
    GetBulletPublic,
    GetBulletPrivate,
}

impl From<Api> for String {
    fn from(item: Api) -> Self {
        String::from(match item {
            Api::Spot(route) => match route {
                Spot::GetAllSymbols => "/api/v2/symbols",
                Spot::GetAllTickers => "/api/v1/market/allTickers",
                Spot::GetBulletPublic => "/api/v1/bullet-public",
                Spot::GetBulletPrivate => "/api/v1/bullet-private",
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

impl Kucoin for BaseInfo {
    fn new(cfg: ClientConfig) -> anyhow::Result<BaseInfo> {
        Ok(BaseInfo {
            client: Client::from_config(cfg)?,
        })
    }
}
