use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

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

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Symbol {
    pub symbol: String,
    pub status: String,
    pub base_asset: String,
    pub base_asset_precision: u32,
    pub quote_asset: String,
    pub quote_precision: u32,
    pub base_commission_precision: u32,
    pub quote_commission_precision: u32,
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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SendOrderRequest {
    pub symbol: String,
    pub order_side: OrderSide,
    pub order_type: OrderType,
    pub time_in_force: Option<TimeInForce>,
    #[serde(with = "rust_decimal::serde::float_option")]
    pub quantity: Option<Decimal>,
    #[serde(with = "rust_decimal::serde::float_option")]
    pub quote_order_qty: Option<Decimal>,
    #[serde(with = "rust_decimal::serde::float_option")]
    pub price: Option<Decimal>,
    pub new_client_order_id: Option<String>,
    pub strategy_id: Option<i64>,
    pub strategy_type: Option<i64>,
    #[serde(with = "rust_decimal::serde::float_option")]
    pub stop_price: Option<Decimal>,
    #[serde(with = "rust_decimal::serde::float_option")]
    pub trailing_delta: Option<Decimal>,
    #[serde(with = "rust_decimal::serde::float_option")]
    pub iceberg_qty: Option<Decimal>,
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
    #[serde(with = "rust_decimal::serde::float")]
    pub price: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    pub orig_qty: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    pub executed_qty: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    pub orig_quote_order_qty: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    pub cummulative_quote_qty: Decimal,
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
    #[serde(with = "rust_decimal::serde::float")]
    pub price: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    pub qty: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    pub commission: Decimal,
    pub commission_asset: String,
    pub trade_id: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct OrderBook {
    pub last_update_id: u64,
    #[serde(rename = "bids")]
    pub bids: Vec<OrderBookUnit>,
    #[serde(rename = "asks")]
    pub asks: Vec<OrderBookUnit>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct OrderBookUnit {
    #[serde(with = "rust_decimal::serde::float")]
    pub price: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    pub qty: Decimal,
}

// #[derive(PartialEq, Debug, Serialize, Deserialize, Clone)]
// pub struct Bids {
//     #[serde(with = "rust_decimal::serde::float")]
//     pub price: Decimal,
//     #[serde(with = "rust_decimal::serde::float")]
//     pub qty: Decimal,
// }
//
// #[derive(Debug, Serialize, Deserialize, Clone)]
// pub struct Asks {
//     #[serde(with = "rust_decimal::serde::float")]
//     pub price: Decimal,
//     #[serde(with = "rust_decimal::serde::float")]
//     pub qty: Decimal,
// }

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
#[serde(default)]
pub struct TickerPriceStats {
    pub symbol: String,
    #[serde(with = "rust_decimal::serde::float_option")]
    pub price_change: Option<Decimal>,
    #[serde(with = "rust_decimal::serde::float_option")]
    pub price_change_percent: Option<Decimal>,
    #[serde(with = "rust_decimal::serde::float_option")]
    pub weighted_avg_price: Option<Decimal>,
    #[serde(with = "rust_decimal::serde::float_option")]
    pub prev_close_price: Option<Decimal>,
    #[serde(with = "rust_decimal::serde::float")]
    pub last_price: Decimal,
    #[serde(with = "rust_decimal::serde::float_option")]
    pub last_qty: Option<Decimal>,
    #[serde(with = "rust_decimal::serde::float_option")]
    pub bid_price: Option<Decimal>,
    #[serde(with = "rust_decimal::serde::float_option")]
    pub bid_qty: Option<Decimal>,
    #[serde(with = "rust_decimal::serde::float_option")]
    pub ask_price: Option<Decimal>,
    #[serde(with = "rust_decimal::serde::float_option")]
    pub ask_qty: Option<Decimal>,
    #[serde(with = "rust_decimal::serde::float")]
    pub open_price: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    pub high_price: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    pub low_price: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    pub volume: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    pub quote_volume: Decimal,
    pub open_time: u64,
    pub close_time: u64,
    pub first_id: u64,
    pub last_id: u64,
    pub count: u64,
}

#[cfg(test)]
mod tests {
    use crate::libs::binance_api::{SendOrderResponse, TickerPriceStats};

    #[test]
    fn test_deserialize_send_order_response() {
        let data = r#"
        {
          "symbol": "BTCUSDT",
          "orderId": 28,
          "orderListId": -1,
          "clientOrderId": "6gCrw2kRUAF9CvJDGP16IP",
          "transactTime": 1507725176595,
          "price": "0.00000000",
          "origQty": "10.00000000",
          "executedQty": "10.00000000",
          "origQuoteOrderQty": "0.000000",
          "cummulativeQuoteQty": "10.00000000",
          "status": "FILLED",
          "timeInForce": "GTC",
          "type": "MARKET",
          "side": "SELL",
          "workingTime": 1507725176595,
          "selfTradePreventionMode": "NONE",
          "fills": [
            {
              "price": "4000.00000000",
              "qty": "1.00000000",
              "commission": "4.00000000",
              "commissionAsset": "USDT",
              "tradeId": 56
            },
            {
              "price": "3999.00000000",
              "qty": "5.00000000",
              "commission": "19.99500000",
              "commissionAsset": "USDT",
              "tradeId": 57
            },
            {
              "price": "3998.00000000",
              "qty": "2.00000000",
              "commission": "7.99600000",
              "commissionAsset": "USDT",
              "tradeId": 58
            },
            {
              "price": "3997.00000000",
              "qty": "1.00000000",
              "commission": "3.99700000",
              "commissionAsset": "USDT",
              "tradeId": 59
            },
            {
              "price": "3995.00000000",
              "qty": "1.00000000",
              "commission": "3.99500000",
              "commissionAsset": "USDT",
              "tradeId": 60
            }
          ]
        }
        "#;

        serde_json::from_str::<SendOrderResponse>(data).unwrap();
    }

    #[test]
    fn test_deserialize_ticker_price_stats() {
        let full_response = r#"
        {
          "symbol": "BNBBTC",
          "priceChange": "-94.99999800",
          "priceChangePercent": "-95.960",
          "weightedAvgPrice": "0.29628482",
          "prevClosePrice": "0.10002000",
          "lastPrice": "4.00000200",
          "lastQty": "200.00000000",
          "bidPrice": "4.00000000",
          "bidQty": "100.00000000",
          "askPrice": "4.00000200",
          "askQty": "100.00000000",
          "openPrice": "99.00000000",
          "highPrice": "100.00000000",
          "lowPrice": "0.10000000",
          "volume": "8913.30000000",
          "quoteVolume": "15.30000000",
          "openTime": 1499783499040,
          "closeTime": 1499869899040,
          "firstId": 28385,
          "lastId": 28460,
          "count": 76
        }
        "#;

        serde_json::from_str::<TickerPriceStats>(full_response).unwrap();

        let mini_response = r#"
        {
            "symbol": "BNBBTC",
            "openPrice": "99.00000000",
            "highPrice": "100.00000000",
            "lowPrice": "0.10000000",
            "lastPrice": "4.00000200",
            "volume": "8913.30000000",
            "quoteVolume": "15.30000000",
            "openTime": 1499783499040,
            "closeTime": 1499869899040,
            "firstId": 28385,
            "lastId": 28460,
            "count": 76
        }
        "#;

        serde_json::from_str::<TickerPriceStats>(mini_response).unwrap();
    }
}
