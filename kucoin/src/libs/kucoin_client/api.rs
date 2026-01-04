use crate::libs::kucoin_client::{BaseInfo, ClientConfig, Market, client::Client};

pub enum Api {
    Spot(Spot),
}

pub enum Spot {
    GetAllSymbols,
    GetAllTickers,
    GetBulletPublic,
    GetBulletPrivate,
}

impl Api {
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Spot(route) => match route {
                Spot::GetAllSymbols => "/api/v2/symbols",
                Spot::GetAllTickers => "/api/v1/market/allTickers",
                Spot::GetBulletPublic => "/api/v1/bullet-public",
                Spot::GetBulletPrivate => "/api/v1/bullet-private",
            },
        }
    }
}

impl From<Api> for String {
    fn from(item: Api) -> Self {
        item.as_str().to_owned()
    }
}

pub trait Kucoin {
    fn new(cfg: ClientConfig) -> anyhow::Result<Self>
    where
        Self: Sized;
}

impl Kucoin for Market {
    fn new(cfg: ClientConfig) -> anyhow::Result<Self> {
        Ok(Self {
            client: Client::from_config(cfg)?,
        })
    }
}

impl Kucoin for BaseInfo {
    fn new(cfg: ClientConfig) -> anyhow::Result<Self> {
        Ok(Self {
            client: Client::from_config(cfg)?,
        })
    }
}
