use serde_derive::{Deserialize, Serialize};

use crate::libs::binance_api::enums::{
    NewOrderRespType, OrderSide, OrderStatus, OrderType, SelfTradePreventionMode, TimeInForce,
};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ExchangeInformation {
    pub timezone: String,
    pub server_time: u64,
    pub symbols: Vec<Symbol>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Symbol {
    pub symbol: String,
    pub status: String,
    pub base_asset: String,
    pub base_asset_precision: u64,
    pub quote_asset: String,
    pub quote_precision: u64,
    pub order_types: Vec<String>,
    pub iceberg_allowed: bool,
    pub is_spot_trading_allowed: bool,
    pub is_margin_trading_allowed: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AccountInformation {
    pub maker_commission: f32,
    pub taker_commission: f32,
    pub buyer_commission: f32,
    pub seller_commission: f32,
    pub can_trade: bool,
    pub can_withdraw: bool,
    pub can_deposit: bool,
    pub balances: Vec<Balance>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Balance {
    pub asset: String,
    pub free: String,
    pub locked: String,
}

pub struct SendOrderRequest {
    pub symbol: String,
    pub order_side: OrderSide,
    pub order_type: OrderType,
    pub quantity: Option<f64>,
    pub quote_order_qty: Option<f64>,
    pub price: Option<f64>,
    pub new_client_order_id: Option<String>,
    pub strategy_id: Option<i64>,
    pub strategy_type: Option<i64>,
    pub stop_price: Option<f64>,
    pub trailing_delta: Option<f64>,
    pub iceberg_qty: Option<f64>,
    pub new_order_resp_type: Option<NewOrderRespType>,
    pub self_trade_prevention_mode: Option<SelfTradePreventionMode>,
    pub recv_window: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SendOrderResponse {
    pub symbol: String,
    pub order_id: u64,
    pub order_list_id: i64,
    pub client_order_id: String,
    pub transact_time: u64,
    pub price: f64,
    pub orig_qty: f64,
    pub executed_qty: f64,
    pub orig_quote_order_qty: f64,
    pub cummulative_quote_qty: f64,
    pub status: OrderStatus,
    pub time_in_force: TimeInForce,
    #[serde(rename = "type")]
    pub order_type: OrderType,
    #[serde(rename = "side")]
    pub order_side: OrderSide,
    pub working_time: u64,
    pub self_trade_prevention_mode: SelfTradePreventionMode,
    pub fills: Vec<FillInfo>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FillInfo {
    pub price: f64,
    pub qty: f64,
    pub commission: f64,
    pub commission_asset: String,
    pub trade_id: u64,
}
