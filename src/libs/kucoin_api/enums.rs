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
