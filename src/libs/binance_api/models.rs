use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::libs::binance_api::enums::{
    NewOrderRespType, OrderSide, OrderStatus, OrderType, SelfTradePreventionMode, SymbolStatus,
    TimeInForce,
};
use crate::libs::binance_api::Filters;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ExchangeInformation {
    pub timezone: String,
    pub server_time: u64,
    pub symbols: Vec<Symbol>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct Symbol {
    pub symbol: String,
    pub status: SymbolStatus,
    pub base_asset: String,
    pub base_asset_precision: u32,
    pub quote_asset: String,
    pub quote_precision: u32,
    pub base_commission_precision: u32,
    pub quote_commission_precision: u32,
    pub order_types: Vec<OrderType>,
    pub iceberg_allowed: bool,
    pub is_spot_trading_allowed: bool,
    pub is_margin_trading_allowed: bool,
    pub filters: Vec<Filters>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AccountInformation {
    pub maker_commission: f32,
    pub taker_commission: f32,
    pub buyer_commission: f32,
    pub seller_commission: f32,
    pub commission_rates: CommissionRates,
    pub can_trade: bool,
    pub can_withdraw: bool,
    pub can_deposit: bool,
    pub balances: Vec<Balance>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CommissionRates {
    pub maker: Decimal,
    pub taker: Decimal,
    pub buyer: Decimal,
    pub seller: Decimal,
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
    use crate::libs::binance_api::{
        AccountInformation, ExchangeInformation, OrderBook, SendOrderResponse, TickerPriceStats,
    };

    #[test]
    fn test_deserialize_exchange_information() {
        let data = r#"
        {
           "timezone":"UTC",
           "serverTime":1753314650438,
           "rateLimits":[
              {
                 "rateLimitType":"REQUEST_WEIGHT",
                 "interval":"MINUTE",
                 "intervalNum":1,
                 "limit":6000
              },
              {
                 "rateLimitType":"ORDERS",
                 "interval":"SECOND",
                 "intervalNum":10,
                 "limit":100
              },
              {
                 "rateLimitType":"ORDERS",
                 "interval":"DAY",
                 "intervalNum":1,
                 "limit":200000
              },
              {
                 "rateLimitType":"RAW_REQUESTS",
                 "interval":"MINUTE",
                 "intervalNum":5,
                 "limit":61000
              }
           ],
           "exchangeFilters":[

           ],
           "symbols":[
              {
                 "symbol":"BTCUSDT",
                 "status":"TRADING",
                 "baseAsset":"BTC",
                 "baseAssetPrecision":8,
                 "quoteAsset":"USDT",
                 "quotePrecision":8,
                 "quoteAssetPrecision":8,
                 "baseCommissionPrecision":8,
                 "quoteCommissionPrecision":8,
                 "orderTypes":[
                    "LIMIT",
                    "LIMIT_MAKER",
                    "MARKET",
                    "STOP_LOSS",
                    "STOP_LOSS_LIMIT",
                    "TAKE_PROFIT",
                    "TAKE_PROFIT_LIMIT"
                 ],
                 "icebergAllowed":true,
                 "ocoAllowed":true,
                 "otoAllowed":true,
                 "quoteOrderQtyMarketAllowed":true,
                 "allowTrailingStop":true,
                 "cancelReplaceAllowed":true,
                 "amendAllowed":true,
                 "isSpotTradingAllowed":true,
                 "isMarginTradingAllowed":true,
                 "filters":[
                    {
                       "filterType":"PRICE_FILTER",
                       "minPrice":"0.01000000",
                       "maxPrice":"1000000.00000000",
                       "tickSize":"0.01000000"
                    },
                    {
                       "filterType":"LOT_SIZE",
                       "minQty":"0.00001000",
                       "maxQty":"9000.00000000",
                       "stepSize":"0.00001000"
                    },
                    {
                       "filterType":"ICEBERG_PARTS",
                       "limit":10
                    },
                    {
                       "filterType":"MARKET_LOT_SIZE",
                       "minQty":"0.00000000",
                       "maxQty":"77.94145208",
                       "stepSize":"0.00000000"
                    },
                    {
                       "filterType":"TRAILING_DELTA",
                       "minTrailingAboveDelta":10,
                       "maxTrailingAboveDelta":2000,
                       "minTrailingBelowDelta":10,
                       "maxTrailingBelowDelta":2000
                    },
                    {
                       "filterType":"PERCENT_PRICE_BY_SIDE",
                       "bidMultiplierUp":"5",
                       "bidMultiplierDown":"0.2",
                       "askMultiplierUp":"5",
                       "askMultiplierDown":"0.2",
                       "avgPriceMins":5
                    },
                    {
                       "filterType":"NOTIONAL",
                       "minNotional":"5.00000000",
                       "applyMinToMarket":true,
                       "maxNotional":"9000000.00000000",
                       "applyMaxToMarket":false,
                       "avgPriceMins":5
                    },
                    {
                       "filterType":"MAX_NUM_ORDERS",
                       "maxNumOrders":200
                    },
                    {
                       "filterType":"MAX_NUM_ALGO_ORDERS",
                       "maxNumAlgoOrders":5
                    }
                 ],
                 "permissions":[

                 ],
                 "permissionSets":[

                 ],
                 "defaultSelfTradePreventionMode":"EXPIRE_MAKER",
                 "allowedSelfTradePreventionModes":[
                    "EXPIRE_TAKER",
                    "EXPIRE_MAKER",
                    "EXPIRE_BOTH",
                    "DECREMENT"
                 ]
              }
           ]
        }
        "#;

        serde_json::from_str::<ExchangeInformation>(data).unwrap();
    }

    #[test]
    fn test_deserialize_account_information() {
        let data = r#"
        {
          "makerCommission": 15,
          "takerCommission": 15,
          "buyerCommission": 0,
          "sellerCommission": 0,
          "commissionRates": {
            "maker": "0.00150000",
            "taker": "0.00150000",
            "buyer": "0.00000000",
            "seller": "0.00000000"
          },
          "canTrade": true,
          "canWithdraw": true,
          "canDeposit": true,
          "brokered": false,
          "requireSelfTradePrevention": false,
          "preventSor": false,
          "updateTime": 123456789,
          "accountType": "SPOT",
          "balances": [
            {
              "asset": "BTC",
              "free": "4723846.89208129",
              "locked": "0.00000000"
            },
            {
              "asset": "LTC",
              "free": "4763368.68006011",
              "locked": "0.00000000"
            }
          ],
          "permissions": [
            "SPOT"
          ],
          "uid": 354937868
        }
        "#;

        serde_json::from_str::<AccountInformation>(data).unwrap();
    }

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
    fn test_deserialize_order_book() {
        let data = r#"
        {
          "lastUpdateId": 1027024,
          "bids": [
            [
              "4.00000000",
              "431.00000000"
            ]
          ],
          "asks": [
            [
              "4.00000200",
              "12.00000000"
            ]
          ]
        }
        "#;

        serde_json::from_str::<OrderBook>(data).unwrap();
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
