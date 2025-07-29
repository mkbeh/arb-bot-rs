use std::fmt::Display;

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
            SymbolStatus::Trading => write!(f, "TRADING"),
            SymbolStatus::EndOfDay => write!(f, "END_OF_DAY"),
            SymbolStatus::Halt => write!(f, "HALT"),
            SymbolStatus::Break => write!(f, "BREAK"),
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
            OrderSide::Buy => write!(f, "BUY"),
            OrderSide::Sell => write!(f, "SELL"),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
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
            OrderType::Limit => write!(f, "LIMIT"),
            OrderType::Market => write!(f, "MARKET"),
            OrderType::StopLoss => write!(f, "STOP_LOSS"),
            OrderType::StopLossLimit => write!(f, "STOP_LOSS_LIMIT"),
            OrderType::TakeProfit => write!(f, "TAKE_PROFIT"),
            OrderType::TakeProfitLimit => write!(f, "TAKE_PROFIT_LIMIT"),
            OrderType::LimitMaker => write!(f, "LIMIT_MAKER"),
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
            TimeInForce::Gtc => write!(f, "GTC"),
            TimeInForce::Ioc => write!(f, "IOC"),
            TimeInForce::Fok => write!(f, "FOK"),
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
            NewOrderRespType::Ack => write!(f, "ACK"),
            NewOrderRespType::Result => write!(f, "RESULT"),
            NewOrderRespType::Full => write!(f, "FULL"),
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
            SelfTradePreventionMode::None => write!(f, "NONE"),
            SelfTradePreventionMode::ExpireMaker => write!(f, "EXPIRE_MAKER"),
            SelfTradePreventionMode::ExpireTaker => write!(f, "EXPIRE_TAKER"),
            SelfTradePreventionMode::ExpireBoth => write!(f, "EXPIRE_BOTH"),
            SelfTradePreventionMode::Decrement => write!(f, "DECREMENT"),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
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
            OrderStatus::Canceled => write!(f, "CANCELED"),
            OrderStatus::Expired => write!(f, "EXPIRED"),
            OrderStatus::Filled => write!(f, "FILLED"),
            OrderStatus::New => write!(f, "NEW"),
            OrderStatus::PartiallyFilled => write!(f, "PARTIALLY_FILLED"),
            OrderStatus::PendingCancel => write!(f, "PENDING_CANCELED"),
            OrderStatus::Rejected => write!(f, "REJECTED"),
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
            TickerPriceResponseType::Full => write!(f, "FULL"),
            TickerPriceResponseType::Mini => write!(f, "MINI"),
        }
    }
}
