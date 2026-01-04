use std::fmt::Display;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum MarketType {
    #[serde(rename = "Hot")]
    Hot,
    #[serde(rename = "AI")]
    Ai,
    #[serde(rename = "ETF-Related")]
    EtfRelated,
    #[serde(rename = "Meme")]
    Meme,
    #[serde(rename = "USDS")]
    Usds,
    #[serde(rename = "TON")]
    Ton,
    #[serde(rename = "ETF")]
    Etf,
    #[serde(rename = "KCS")]
    Stocks,
    #[serde(rename = "Stocks")]
    Kcs,
    #[serde(rename = "Solana")]
    Solana,
    #[serde(rename = "FIAT")]
    Fiat,
    #[serde(rename = "DeFi")]
    Defi,
    #[serde(rename = "BTC")]
    Btc,
    #[serde(rename = "ALTS")]
    Alts,
    #[serde(rename = "VR_AR")]
    VrAr,
    #[serde(rename = "NFT")]
    Nft,
}

impl MarketType {
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Hot => "Hot",
            Self::Ai => "AI",
            Self::EtfRelated => "ETF-Related",
            Self::Meme => "Meme",
            Self::Usds => "USDS",
            Self::Ton => "TON",
            Self::Etf => "ETF",
            Self::Stocks => "Stocks",
            Self::Kcs => "KCS",
            Self::Solana => "Solana",
            Self::Fiat => "FIAT",
            Self::Defi => "DeFi",
            Self::Btc => "BTC",
            Self::Alts => "ALTS",
            Self::VrAr => "VR_AR",
            Self::Nft => "NFT",
        }
    }
}

impl Display for MarketType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum OrderSide {
    Buy,
    Sell,
}

impl Display for OrderSide {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Buy => write!(f, "buy"),
            Self::Sell => write!(f, "sell"),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum OrderType {
    Limit,
    Market,
}

impl Display for OrderType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Limit => write!(f, "limit"),
            Self::Market => write!(f, "market"),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum OrderStatus {
    New,
    Open,
    Match,
    Done,
}

impl Display for OrderStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::New => write!(f, "new"),
            Self::Open => write!(f, "open"),
            Self::Match => write!(f, "match"),
            Self::Done => write!(f, "done"),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum OrderChangeType {
    Open,
    Match,
    Update,
    Filled,
    Canceled,
    Received,
}

impl Display for OrderChangeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Open => write!(f, "open"),
            Self::Match => write!(f, "match"),
            Self::Update => write!(f, "update"),
            Self::Filled => write!(f, "filled"),
            Self::Canceled => write!(f, "canceled"),
            Self::Received => write!(f, "received"),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FeeType {
    TakerFee,
    MakerFee,
}

impl Display for FeeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TakerFee => write!(f, "takerFee"),
            Self::MakerFee => write!(f, "makerFee"),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Liquidity {
    Taker,
    Maker,
}

impl Display for Liquidity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Taker => write!(f, "taker"),
            Self::Maker => write!(f, "maker"),
        }
    }
}
