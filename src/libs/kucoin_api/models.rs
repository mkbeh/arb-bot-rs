use rust_decimal::Decimal;
use serde::Deserialize;

use crate::libs::kucoin_api::enums::MarketType;

#[derive(Deserialize, Debug, Clone)]
pub struct RestResponse<T> {
    pub code: String,
    pub data: T,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Symbol {
    pub symbol: String,
    pub name: String,
    pub base_currency: String,
    pub quote_currency: String,
    pub fee_currency: String,
    pub market: MarketType,
    #[serde(with = "rust_decimal::serde::float")]
    pub base_min_size: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    pub quote_min_size: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    pub base_max_size: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    pub quote_max_size: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    pub base_increment: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    pub quote_increment: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    pub price_increment: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    pub price_limit_rate: Decimal,
    #[serde(with = "rust_decimal::serde::float_option")]
    pub min_funds: Option<Decimal>,
    pub is_margin_enabled: bool,
    pub enable_trading: bool,
    pub fee_category: i32,
    #[serde(with = "rust_decimal::serde::float")]
    pub maker_fee_coefficient: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    pub taker_fee_coefficient: Decimal,
    pub st: bool,
    pub callauction_is_enabled: bool,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Token {
    pub token: String,
    pub instance_servers: Vec<InstanceServer>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct InstanceServer {
    pub endpoint: String,
    pub encrypt: bool,
    pub protocol: String,
    pub ping_interval: u64,
    pub ping_timeout: u64,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AllTickers {
    pub time: u64,
    pub ticker: Vec<Ticker>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Ticker {
    pub symbol: String,
    pub symbol_name: String,
    #[serde(with = "rust_decimal::serde::float_option")]
    pub buy: Option<Decimal>,
    #[serde(with = "rust_decimal::serde::float_option")]
    pub best_bid_size: Option<Decimal>,
    #[serde(with = "rust_decimal::serde::float_option")]
    pub sell: Option<Decimal>,
    #[serde(with = "rust_decimal::serde::float_option")]
    pub best_ask_size: Option<Decimal>,
    #[serde(with = "rust_decimal::serde::float_option")]
    pub change_rate: Option<Decimal>,
    #[serde(with = "rust_decimal::serde::float")]
    pub high: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    pub low: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    pub vol: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    pub vol_value: Decimal,
    #[serde(with = "rust_decimal::serde::float_option")]
    pub last: Option<Decimal>,
    #[serde(with = "rust_decimal::serde::float_option")]
    pub change_price: Option<Decimal>,
    #[serde(with = "rust_decimal::serde::float_option")]
    pub average_price: Option<Decimal>,
    #[serde(with = "rust_decimal::serde::float")]
    pub taker_fee_rate: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    pub maker_fee_rate: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    pub taker_coefficient: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    pub maker_coefficient: Decimal,
}
