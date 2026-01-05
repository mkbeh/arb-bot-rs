use std::fmt::Display;

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SymbolStatus {
    #[default]
    Trading,
    EndOfDay,
    Halt,
    Break,
}

impl Display for SymbolStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Trading => write!(f, "TRADING"),
            Self::EndOfDay => write!(f, "END_OF_DAY"),
            Self::Halt => write!(f, "HALT"),
            Self::Break => write!(f, "BREAK"),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OrderSide {
    Buy,
    Sell,
}

impl Display for OrderSide {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Buy => write!(f, "BUY"),
            Self::Sell => write!(f, "SELL"),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OrderType {
    Limit,
    Market,
    StopLoss,
    StopLossLimit,
    TakeProfit,
    TakeProfitLimit,
    LimitMaker,
}

impl Display for OrderType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Limit => write!(f, "LIMIT"),
            Self::Market => write!(f, "MARKET"),
            Self::StopLoss => write!(f, "STOP_LOSS"),
            Self::StopLossLimit => write!(f, "STOP_LOSS_LIMIT"),
            Self::TakeProfit => write!(f, "TAKE_PROFIT"),
            Self::TakeProfitLimit => write!(f, "TAKE_PROFIT_LIMIT"),
            Self::LimitMaker => write!(f, "LIMIT_MAKER"),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TimeInForce {
    Gtc,
    Ioc,
    Fok,
}

impl Display for TimeInForce {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Gtc => write!(f, "GTC"),
            Self::Ioc => write!(f, "IOC"),
            Self::Fok => write!(f, "FOK"),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NewOrderRespType {
    Ack,
    Result,
    Full,
}

impl Display for NewOrderRespType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ack => write!(f, "ACK"),
            Self::Result => write!(f, "RESULT"),
            Self::Full => write!(f, "FULL"),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SelfTradePreventionMode {
    None,
    ExpireMaker,
    ExpireTaker,
    ExpireBoth,
    Decrement,
}

impl Display for SelfTradePreventionMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => write!(f, "NONE"),
            Self::ExpireMaker => write!(f, "EXPIRE_MAKER"),
            Self::ExpireTaker => write!(f, "EXPIRE_TAKER"),
            Self::ExpireBoth => write!(f, "EXPIRE_BOTH"),
            Self::Decrement => write!(f, "DECREMENT"),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OrderStatus {
    Canceled,
    Expired,
    Filled,
    New,
    PartiallyFilled,
    PendingCancel,
    Rejected,
}

impl Display for OrderStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Canceled => write!(f, "CANCELED"),
            Self::Expired => write!(f, "EXPIRED"),
            Self::Filled => write!(f, "FILLED"),
            Self::New => write!(f, "NEW"),
            Self::PartiallyFilled => write!(f, "PARTIALLY_FILLED"),
            Self::PendingCancel => write!(f, "PENDING_CANCELED"),
            Self::Rejected => write!(f, "REJECTED"),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TickerPriceResponseType {
    Full,
    Mini,
}

impl Display for TickerPriceResponseType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Full => write!(f, "FULL"),
            Self::Mini => write!(f, "MINI"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(tag = "filterType")]
pub enum Filters {
    #[serde(rename = "PRICE_FILTER")]
    #[serde(rename_all = "camelCase")]
    PriceFilter {
        #[serde(with = "rust_decimal::serde::float")]
        min_price: Decimal,
        #[serde(with = "rust_decimal::serde::float")]
        max_price: Decimal,
        #[serde(with = "rust_decimal::serde::float")]
        tick_size: Decimal,
    },
    #[serde(rename = "PERCENT_PRICE")]
    #[serde(rename_all = "camelCase")]
    PercentPrice {
        multiplier_up: String,
        multiplier_down: String,
        #[serde(with = "rust_decimal::serde::float_option")]
        avg_price_mins: Option<Decimal>,
    },
    #[serde(rename = "PERCENT_PRICE_BY_SIDE")]
    #[serde(rename_all = "camelCase")]
    PercentPriceBySide {
        bid_multiplier_up: String,
        bid_multiplier_down: String,
        ask_multiplier_up: String,
        ask_multiplier_down: String,
        #[serde(with = "rust_decimal::serde::float_option")]
        avg_price_mins: Option<Decimal>,
    },
    #[serde(rename = "LOT_SIZE")]
    #[serde(rename_all = "camelCase")]
    LotSize {
        min_qty: Decimal,
        max_qty: Decimal,
        step_size: Decimal,
    },
    #[serde(rename = "MIN_NOTIONAL")]
    #[serde(rename_all = "camelCase")]
    MinNotional {
        #[serde(with = "rust_decimal::serde::float_option")]
        notional: Option<Decimal>,
        #[serde(with = "rust_decimal::serde::float_option")]
        min_notional: Option<Decimal>,
        apply_to_market: Option<bool>,
        #[serde(with = "rust_decimal::serde::float_option")]
        avg_price_mins: Option<Decimal>,
    },
    #[serde(rename = "NOTIONAL")]
    #[serde(rename_all = "camelCase")]
    Notional {
        #[serde(with = "rust_decimal::serde::float_option")]
        min_notional: Option<Decimal>,
        apply_min_to_market: Option<bool>,
        #[serde(with = "rust_decimal::serde::float_option")]
        max_notional: Option<Decimal>,
        apply_max_to_market: Option<bool>,
        #[serde(with = "rust_decimal::serde::float_option")]
        avg_price_mins: Option<Decimal>,
    },
    #[serde(rename = "ICEBERG_PARTS")]
    #[serde(rename_all = "camelCase")]
    IcebergParts { limit: Option<u16> },
    #[serde(rename = "MARKET_LOT_SIZE")]
    #[serde(rename_all = "camelCase")]
    MarketLotSize {
        min_qty: String,
        max_qty: String,
        step_size: String,
    },
    #[serde(rename = "MAX_NUM_ORDERS")]
    #[serde(rename_all = "camelCase")]
    MaxNumOrders { max_num_orders: Option<u16> },
    #[serde(rename = "MAX_NUM_ALGO_ORDERS")]
    #[serde(rename_all = "camelCase")]
    MaxNumAlgoOrders { max_num_algo_orders: Option<u16> },
    #[serde(rename = "MAX_NUM_ICEBERG_ORDERS")]
    #[serde(rename_all = "camelCase")]
    MaxNumIcebergOrders { max_num_iceberg_orders: u16 },
    #[serde(rename = "MAX_POSITION")]
    #[serde(rename_all = "camelCase")]
    MaxPosition { max_position: String },
    #[serde(rename = "TRAILING_DELTA")]
    #[serde(rename_all = "camelCase")]
    TrailingData {
        min_trailing_above_delta: Option<u16>,
        max_trailing_above_delta: Option<u16>,
        min_trailing_below_delta: Option<u16>,
        max_trailing_below_delta: Option<u16>,
    },
    #[serde(rename = "MAX_NUM_ORDER_AMENDS")]
    #[serde(rename_all = "camelCase")]
    MaxNumOrderAmends { max_num_order_amends: u16 },
    #[serde(rename = "MAX_NUM_ORDER_LISTS")]
    #[serde(rename_all = "camelCase")]
    MaxNumOrderLists { max_num_order_lists: u16 },
}
