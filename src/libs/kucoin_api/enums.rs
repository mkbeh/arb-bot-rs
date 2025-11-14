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
    pub const fn as_str(&self) -> &'static str {
        match self {
            MarketType::Hot => "Hot",
            MarketType::Ai => "AI",
            MarketType::EtfRelated => "ETF-Related",
            MarketType::Meme => "Meme",
            MarketType::Usds => "USDS",
            MarketType::Ton => "TON",
            MarketType::Etf => "ETF",
            MarketType::Stocks => "Stocks",
            MarketType::Kcs => "KCS",
            MarketType::Solana => "Solana",
            MarketType::Fiat => "FIAT",
            MarketType::Defi => "DeFi",
            MarketType::Btc => "BTC",
            MarketType::Alts => "ALTS",
            MarketType::VrAr => "VR_AR",
            MarketType::Nft => "NFT",
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
            OrderSide::Buy => write!(f, "buy"),
            OrderSide::Sell => write!(f, "sell"),
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
            OrderType::Limit => write!(f, "limit"),
            OrderType::Market => write!(f, "market"),
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
            OrderStatus::New => write!(f, "new"),
            OrderStatus::Open => write!(f, "open"),
            OrderStatus::Match => write!(f, "match"),
            OrderStatus::Done => write!(f, "done"),
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
            OrderChangeType::Open => write!(f, "open"),
            OrderChangeType::Match => write!(f, "match"),
            OrderChangeType::Update => write!(f, "update"),
            OrderChangeType::Filled => write!(f, "filled"),
            OrderChangeType::Canceled => write!(f, "canceled"),
            OrderChangeType::Received => write!(f, "received"),
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
            FeeType::TakerFee => write!(f, "takerFee"),
            FeeType::MakerFee => write!(f, "makerFee"),
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
            Liquidity::Taker => write!(f, "taker"),
            Liquidity::Maker => write!(f, "maker"),
        }
    }
}
