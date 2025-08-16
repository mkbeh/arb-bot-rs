use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::SystemTime,
};

use anyhow::{anyhow, bail};
use futures_util::{
    FutureExt, SinkExt, StreamExt,
    stream::{SplitSink, SplitStream},
};
use hmac::{Hmac, Mac};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;
use serde_with::skip_serializing_none;
use sha2::Sha256;
use tokio::{net::TcpStream, sync::Mutex};
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, connect_async,
    tungstenite::{Message, Utf8Bytes, handshake::client::Response},
};
use tracing::{error, info};
use url::Url;
use urlencoding::encode;
use uuid::Uuid;

use crate::libs::binance_api::{
    NewOrderRespType, OrderSide, OrderType, SelfTradePreventionMode, TimeInForce, utils,
};

type HmacSha256 = Hmac<Sha256>;

#[derive(Clone)]
pub struct WebsocketClientConfig {
    pub ws_url: String,
    pub api_key: String,
    pub secret_key: String,
}

pub struct WebsocketClient {
    ws_url: String,
    api_key: String,
    secret_key: String,
    stream: Option<WebSocketStream<MaybeTlsStream<TcpStream>>>,
}

impl WebsocketClient {
    pub fn new(config: WebsocketClientConfig) -> Self {
        Self {
            ws_url: config.ws_url,
            api_key: config.api_key,
            secret_key: config.secret_key,
            stream: None,
        }
    }

    pub async fn connect(&mut self) -> anyhow::Result<()> {
        let url = Url::parse(self.ws_url.as_str())?;
        match connect_async(url.as_str()).await {
            Ok((ws_stream, _)) => {
                self.stream = Some(ws_stream);
                Ok(())
            }
            Err(e) => bail!("Failed to connect to websocket: {}", e),
        }
    }

    pub async fn place_order(
        &mut self,
        mut request: PlaceOrderRequest,
    ) -> anyhow::Result<(String)> {
        let mut params: Vec<(String, String)> = Vec::new();

        let timestamp = utils::get_timestamp(SystemTime::now())?;

        params.push(("apiKey".to_owned(), self.api_key.clone()));
        params.push(("side".to_owned(), request.order_side.to_string()));
        params.push(("symbol".to_owned(), request.symbol.clone()));
        params.push(("timestamp".to_owned(), timestamp.to_string()));
        params.push(("type".to_owned(), request.order_type.to_string()));

        // Опциональные параметры (в алфавитном порядке)
        if let Some(ref v) = request.iceberg_qty {
            params.push(("icebergQty".to_owned(), v.to_string()));
        }
        if let Some(ref v) = request.new_client_order_id {
            params.push(("newClientOrderId".to_owned(), v.to_string()));
        }
        if let Some(ref v) = request.new_order_resp_type {
            params.push(("newOrderRespType".to_owned(), v.to_string()));
        }
        if let Some(ref v) = request.price {
            params.push(("price".to_owned(), v.to_string()));
        }
        if let Some(ref v) = request.quantity {
            params.push(("quantity".to_owned(), v.to_string()));
        }
        if let Some(ref v) = request.quote_order_qty {
            params.push(("quoteOrderQty".to_owned(), v.to_string()));
        }
        if let Some(v) = request.recv_window {
            params.push(("recvWindow".to_owned(), v.to_string()));
        }
        if let Some(ref v) = request.self_trade_prevention_mode {
            params.push(("selfTradePreventionMode".to_owned(), v.to_string()));
        }
        if let Some(ref v) = request.stop_price {
            params.push(("stopPrice".to_owned(), v.to_string()));
        }
        if let Some(v) = request.strategy_id {
            params.push(("strategyId".to_owned(), v.to_string()));
        }
        if let Some(v) = request.strategy_type {
            params.push(("strategyType".to_owned(), v.to_string()));
        }
        if let Some(ref v) = request.time_in_force {
            params.push(("timeInForce".to_owned(), v.to_string()));
        }
        if let Some(ref v) = request.trailing_delta {
            params.push(("trailingDelta".to_owned(), v.to_string()));
        }

        params.sort_by(|a, b| a.0.cmp(&b.0));

        let query = build_query_string(params);
        println!("{query}");
        let signature = generate_signature(&self.secret_key, &query);

        request.timestamp = Some(timestamp);
        request.api_key = Some(self.api_key.clone());
        request.signature = Some(signature);

        let request = WebsocketRequest::new(WebsocketApi::PlaceOrder, request);
        let payload =
            serde_json::to_string(&request).map_err(|e| anyhow!("Failed to serialize: {}", e))?;
        println!("{payload}");

        Ok(payload)

        // self.send_request(WebsocketApi::PlaceOrder, request).await?;

        // Ok(())
    }

    pub async fn handle_message(&mut self, message: Message) -> anyhow::Result<()> {
        if let Some(ref mut stream) = self.stream {
            match message {
                Message::Text(msg) => match serde_json::from_str::<Value>(&msg) {
                    Ok(json) => {
                        info!("Received: {}", json);
                    }
                    Err(e) => {
                        error!("Parse error: {}", e);
                        return Err(anyhow::anyhow!("Parse error: {}", e));
                    }
                },
                Message::Ping(ping) => {
                    if let Err(e) = stream.send(Message::Pong(ping.into())).await {
                        error!("Pong failed: {}", e);
                        return Err(anyhow::anyhow!("Pong failed: {e}"));
                    };
                    println!("ping ok")
                }
                Message::Close(_) => {
                    info!("Connection closed");
                    return Err(anyhow::anyhow!("Connection closed"));
                }
                _ => {}
            }
        };

        Ok(())
    }

    async fn send_request<T>(&mut self, method: WebsocketApi, params: T) -> anyhow::Result<()>
    where
        T: Serialize,
    {
        let request = WebsocketRequest::new(method, params);
        let payload =
            serde_json::to_string(&request).map_err(|e| anyhow!("Failed to serialize: {}", e))?;

        if let Some(ref mut stream) = self.stream {
            stream
                .send(Message::Text(payload.into()))
                .await
                .map_err(|e| anyhow!("Failed to send websocket request: {}", e))?;
        }

        Ok(())
    }
}

fn build_query_string(params: Vec<(String, String)>) -> String {
    params
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("&")
}

fn generate_signature(secret: &str, query: &str) -> String {
    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("invalid length of secret key");
    mac.update(query.as_bytes());
    hex::encode(mac.finalize().into_bytes())

    // let mut sign_key =
    //     Hmac::<Sha256>::new_from_slice(secret.as_bytes()).expect("invalid length of secret key");
    // sign_key.update(query.as_bytes());
    // hex::encode(sign_key.finalize().into_bytes())
}

enum WebsocketApi {
    PlaceOrder,
}

impl From<WebsocketApi> for String {
    fn from(api: WebsocketApi) -> String {
        String::from(match api {
            WebsocketApi::PlaceOrder => "order.place",
        })
    }
}

#[derive(Serialize)]
struct WebsocketRequest<T>
where
    T: Serialize,
{
    id: String,
    method: String,
    params: T,
}

impl<T> WebsocketRequest<T>
where
    T: Serialize,
{
    fn new(method: WebsocketApi, params: T) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            method: method.into(),
            params,
        }
    }
}

// #[skip_serializing_none]
// #[derive(Debug, Serialize, Clone)]
// #[serde(rename_all = "camelCase")]
// pub struct PlaceOrderRequest {
//     pub symbol: String,
//     #[serde(rename = "side")]
//     pub order_side: OrderSide,
//     #[serde(rename = "type")]
//     pub order_type: OrderType,
//     pub time_in_force: Option<TimeInForce>,
//     #[serde(with = "rust_decimal::serde::float_option")]
//     pub quantity: Option<Decimal>,
//     #[serde(with = "rust_decimal::serde::float_option")]
//     pub quote_order_qty: Option<Decimal>,
//     #[serde(with = "rust_decimal::serde::float_option")]
//     pub price: Option<Decimal>,
//     pub new_client_order_id: Option<String>,
//     pub strategy_id: Option<i64>,
//     pub strategy_type: Option<i64>,
//     #[serde(with = "rust_decimal::serde::float_option")]
//     pub stop_price: Option<Decimal>,
//     #[serde(with = "rust_decimal::serde::float_option")]
//     pub trailing_delta: Option<Decimal>,
//     #[serde(with = "rust_decimal::serde::float_option")]
//     pub iceberg_qty: Option<Decimal>,
//     pub new_order_resp_type: Option<NewOrderRespType>,
//     pub self_trade_prevention_mode: Option<SelfTradePreventionMode>,
//     pub recv_window: Option<u64>,
//     pub api_key: Option<String>,
//     pub timestamp: Option<u64>,
//     pub signature: Option<String>,
// }

#[skip_serializing_none]
#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PlaceOrderRequest {
    pub symbol: String,
    #[serde(rename = "side")]
    pub order_side: OrderSide,
    #[serde(rename = "type")]
    pub order_type: OrderType,
    pub time_in_force: Option<TimeInForce>,
    pub quantity: Option<String>,
    pub quote_order_qty: Option<String>,
    pub price: Option<String>,
    pub new_client_order_id: Option<String>,
    pub strategy_id: Option<i64>,
    pub strategy_type: Option<i64>,
    pub stop_price: Option<String>,
    pub trailing_delta: Option<String>,
    pub iceberg_qty: Option<String>,
    pub new_order_resp_type: Option<NewOrderRespType>,
    pub self_trade_prevention_mode: Option<SelfTradePreventionMode>,
    pub recv_window: Option<u64>,
    pub api_key: Option<String>,
    pub timestamp: Option<u64>,
    pub signature: Option<String>,
}
