//! Binance WebSocket client for authenticated trading operations.
//!
//! # Usage
//!
//! ```rust,no_run
//! use anyhow::Result;
//! use binance::libs::binance_client::{
//!     OrderSide, OrderType,
//!     ws::{ConnectConfig, WebsocketReader, WebsocketWriter, connect_ws},
//! };
//! use tokio_util::sync::CancellationToken;
//!
//! #[tokio::main]
//! async fn main() -> Result<()> {
//!     let config = ConnectConfig::new(
//!         "wss://ws-api.binance.com:443/ws-api/v3".to_string(),
//!         "your-api-key".to_string(),
//!         "your-secret-key".to_string(),
//!     );
//!
//!     let (mut writer, reader) = connect_ws(config).await?;
//!
//!     // Spawn reader task.
//!     let token = CancellationToken::new();
//!     let reader_token = token.clone();
//!     let reader_handle = tokio::spawn(async move {
//!         if let Err(e) = reader.handle_messages(reader_token).await {
//!             eprintln!("Reader error: {}", e);
//!         }
//!     });
//!
//!     // Cancel on completion.
//!     token.cancel();
//!     reader_handle.await?;
//!     Ok(())
//! }
//! ```

use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, SystemTime},
};

use anyhow::{anyhow, bail};
use futures_util::{
    SinkExt, StreamExt,
    stream::{SplitSink, SplitStream},
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;
use serde_with::skip_serializing_none;
use tokio::{
    net::TcpStream,
    sync::{Mutex, mpsc},
};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async, tungstenite::Message};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error};
use url::Url;
use uuid::Uuid;

use crate::libs::binance_client::{
    FillInfo, NewOrderRespType, OrderSide, OrderStatus, OrderType, SelfTradePreventionMode,
    TimeInForce, utils, utils::generate_signature,
};

/// Type alias for the underlying WebSocket stream type.
type WebSocketStreamType = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// Type alias for the WebSocket sink (writer).
type WebSocketSink = SplitSink<WebSocketStreamType, Message>;

/// Type alias for the WebSocket stream (reader).
type WebSocketStreamSplit = SplitStream<WebSocketStreamType>;

/// Type alias for tracking pending requests with response channels.
type PendingRequests = HashMap<String, mpsc::Sender<anyhow::Result<WebsocketResponse<Value>>>>;

/// Configuration for establishing a WebSocket connection to Binance API.
#[derive(Clone)]
pub struct ConnectConfig {
    pub ws_url: String,
    pub api_key: String,
    pub secret_key: String,
}

/// Writer half of the split WebSocket stream for sending authenticated requests.
#[derive(Clone)]
pub struct WebsocketWriter {
    writer: Arc<Mutex<WebSocketSink>>,
    api_key: String,
    secret_key: String,
    response_timeout: Duration,
    pending_requests: Arc<Mutex<PendingRequests>>,
}

/// Reader half of the split WebSocket stream for handling incoming messages.
pub struct WebsocketReader {
    writer: Arc<Mutex<WebSocketSink>>,
    reader: WebSocketStreamSplit,
    pending_requests: Arc<Mutex<PendingRequests>>,
}

/// Establishes a WebSocket connection to Binance and splits into reader/writer halves.
pub async fn connect_ws(conf: ConnectConfig) -> anyhow::Result<(WebsocketWriter, WebsocketReader)> {
    let url = Url::parse(conf.ws_url.as_str())?;
    let (ws_stream, _) = connect_async(url.as_str()).await?;
    let (writer, reader) = ws_stream.split();
    let pending_requests = Arc::new(Mutex::new(HashMap::new()));

    let ws_writer = WebsocketWriter {
        writer: Arc::new(Mutex::new(writer)),
        api_key: conf.api_key,
        secret_key: conf.secret_key,
        response_timeout: Duration::from_secs(10),
        pending_requests: pending_requests.clone(),
    };

    let ws_reader = WebsocketReader {
        writer: ws_writer.writer.clone(),
        reader,
        pending_requests,
    };

    Ok((ws_writer, ws_reader))
}

impl ConnectConfig {
    #[must_use]
    pub fn new(ws_url: String, api_key: String, secret_key: String) -> Self {
        Self {
            ws_url,
            api_key,
            secret_key,
        }
    }
}

impl WebsocketWriter {
    /// Send in a new order.
    pub async fn place_order(
        &mut self,
        mut request: PlaceOrderRequest,
    ) -> anyhow::Result<PlaceOrderResponse> {
        let mut params: Vec<(String, String)> = Vec::new();

        let timestamp = utils::get_timestamp(SystemTime::now())?;

        params.push(("apiKey".to_owned(), self.api_key.clone()));
        params.push(("side".to_owned(), request.order_side.to_string()));
        params.push(("symbol".to_owned(), request.symbol.clone()));
        params.push(("timestamp".to_owned(), timestamp.to_string()));
        params.push(("type".to_owned(), request.order_type.to_string()));

        if let Some(ref v) = request.iceberg_qty {
            params.push(("icebergQty".to_owned(), v.clone()));
        }
        if let Some(ref v) = request.new_client_order_id {
            params.push(("newClientOrderId".to_owned(), v.clone()));
        }
        if let Some(ref v) = request.new_order_resp_type {
            params.push(("newOrderRespType".to_owned(), v.to_string()));
        }
        if let Some(ref v) = request.price {
            params.push(("price".to_owned(), v.clone()));
        }
        if let Some(ref v) = request.quantity {
            params.push(("quantity".to_owned(), v.clone()));
        }
        if let Some(ref v) = request.quote_order_qty {
            params.push(("quoteOrderQty".to_owned(), v.clone()));
        }
        if let Some(v) = request.recv_window {
            params.push(("recvWindow".to_owned(), v.to_string()));
        }
        if let Some(ref v) = request.self_trade_prevention_mode {
            params.push(("selfTradePreventionMode".to_owned(), v.to_string()));
        }
        if let Some(ref v) = request.stop_price {
            params.push(("stopPrice".to_owned(), v.clone()));
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
            params.push(("trailingDelta".to_owned(), v.clone()));
        }

        params.sort_by(|a, b| a.0.cmp(&b.0));

        let query = build_query_string(&params);
        let signature = generate_signature(&self.secret_key, Some(&query));

        request.timestamp = Some(timestamp);
        request.api_key = Some(self.api_key.clone());
        request.signature = Some(signature);

        self.send_request::<PlaceOrderRequest, PlaceOrderResponse>(
            WebsocketApi::PlaceOrder,
            request,
        )
        .await
    }

    /// Check execution status of an order.
    pub async fn query_order(
        &mut self,
        mut request: QueryOrderRequest,
    ) -> anyhow::Result<QueryOrderResponse> {
        let mut params: Vec<(String, String)> = Vec::new();

        let timestamp = utils::get_timestamp(SystemTime::now())?;

        params.push(("apiKey".to_owned(), self.api_key.clone()));
        params.push(("symbol".to_owned(), request.symbol.clone()));
        params.push(("timestamp".to_owned(), timestamp.to_string()));

        if let Some(v) = request.order_id {
            params.push(("orderId".to_owned(), v.to_string()));
        }

        if let Some(ref v) = request.orig_client_order_id {
            params.push(("origClientOrderId".to_owned(), v.into()));
        }

        if let Some(v) = request.recv_window {
            params.push(("recvWindow".to_owned(), v.to_string()));
        }

        params.sort_by(|a, b| a.0.cmp(&b.0));

        let query = build_query_string(&params);
        let signature = generate_signature(&self.secret_key, Some(&query));

        request.timestamp = Some(timestamp);
        request.api_key = Some(self.api_key.clone());
        request.signature = Some(signature);

        self.send_request::<QueryOrderRequest, QueryOrderResponse>(
            WebsocketApi::QueryOrder,
            request,
        )
        .await
    }

    /// Sends a generic signed request over the WebSocket and awaits the response.
    async fn send_request<T, R>(&self, method: WebsocketApi, params: T) -> anyhow::Result<R>
    where
        T: Serialize,
        R: DeserializeOwned,
    {
        let request = WebsocketRequest::new(method, params);

        // Create channel to wait for response
        let (response_tx, mut response_rx) = mpsc::channel(1);

        // Save sender for this request
        {
            let mut pending = self.pending_requests.lock().await;
            pending.insert(request.id.clone(), response_tx);
        }

        // Send request
        let payload = serde_json::to_string(&request)
            .map_err(|e| WebsocketClientError::SerializationError(e.to_string()))?;

        {
            let mut writer = self.writer.lock().await;
            writer
                .send(Message::Text(payload.into()))
                .await
                .map_err(|e| WebsocketClientError::ConnectionError(e.to_string()))?;
        }

        // Wait response with timeout
        match tokio::time::timeout(self.response_timeout, async { response_rx.recv().await }).await
        {
            Ok(Some(Ok(response))) => response.content.into_result(),
            Ok(Some(Err(e))) => Err(WebsocketClientError::RemoteError(e.to_string()).into()),
            Ok(None) => Err(WebsocketClientError::NoResponse.into()),
            Err(_) => {
                // Remove request from pending due to timeout
                let mut pending = self.pending_requests.lock().await;
                pending.remove(&request.id);
                Err(WebsocketClientError::Timeout(self.response_timeout).into())
            }
        }
    }

    pub async fn disconnect(&mut self) {
        let mut w = self.writer.lock().await;
        let _ = w.send(Message::Close(None)).await;
    }
}

impl WebsocketReader {
    /// Handles incoming WebSocket messages in a loop until cancellation or closure.
    pub async fn handle_messages(mut self, token: CancellationToken) -> anyhow::Result<()> {
        loop {
            tokio::select! {
                _ = token.cancelled() => {
                    break;
                }
                message = self.reader.next() => {
                    match message {
                        Some(Ok(Message::Text(text))) => {
                            self.handle_text_message(&text).await?;
                        }
                        Some(Ok(Message::Ping(data))) => {
                            let mut writer = self.writer.lock().await;
                            writer.send(Message::Pong(data)).await
                                .map_err(|e| anyhow!("Failed to send pong: {e}"))?;
                        }
                        Some(Ok(Message::Close(frame))) => {
                            debug!("WebSocket closed: {:?}", frame);
                            break;
                        }
                        Some(Err(e)) => {
                            error!("WebSocket error: {}", e);
                            break;
                        }
                        None => {
                            debug!("WebSocket stream ended");
                            break;
                        }
                        _ => {}
                    }
                }
            }
        }
        Ok(())
    }

    /// Deserializes a text message as a WebSocket response and routes to the matching pending
    /// request.
    async fn handle_text_message(&self, text: &str) -> anyhow::Result<()> {
        match serde_json::from_str::<WebsocketResponse<Value>>(text) {
            Ok(response) => {
                let id = response.id.clone();
                let mut pending = self.pending_requests.lock().await;
                if let Some(response_tx) = pending.remove(&id) {
                    let _ = response_tx.send(Ok(response)).await;
                } else {
                    debug!(request_id = ?id, "Received response for unknown request ID")
                }
            }
            Err(e) => {
                error!(error = ?e, response = ?text, "Failed to parse message");
                bail!("Failed to parse message: {text:?}");
            }
        }
        Ok(())
    }
}

/// Builds a query string from a vector of sorted key-value pairs.
fn build_query_string(params: &[(String, String)]) -> String {
    params
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("&")
}

/// Custom error enum for WebSocket client operations.
#[derive(Debug, thiserror::Error)]
pub enum WebsocketClientError {
    #[error("Request timed out after {0:?}")]
    Timeout(Duration),
    #[error("No response received")]
    NoResponse,
    #[error("Received error: {0}")]
    RemoteError(String),
    #[error("Websocket error: {0}")]
    ConnectionError(String),
    #[error("Serialization error: {0}")]
    SerializationError(String),
}

/// Enum representing supported WebSocket API methods.
pub enum WebsocketApi {
    PlaceOrder,
    QueryOrder,
}

impl From<WebsocketApi> for String {
    fn from(api: WebsocketApi) -> Self {
        Self::from(match api {
            WebsocketApi::PlaceOrder => "order.place",
            WebsocketApi::QueryOrder => "order.status",
        })
    }
}

impl WebsocketApi {
    #[must_use]
    pub fn weight(&self) -> u16 {
        match self {
            Self::PlaceOrder => 1,
            Self::QueryOrder => 4,
        }
    }
}

/// Internal request structure sent over the WebSocket.
#[derive(Debug, Clone, Serialize)]
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

/// Internal response structure received from the WebSocket.
#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
struct WebsocketResponse<T> {
    id: String,
    status: usize,
    #[serde(flatten)]
    content: ResponseContent<T>,
}

/// Enum for response content.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum ResponseContent<T> {
    Success { result: T },
    Error { error: WebsocketError },
}

impl<T> ResponseContent<T> {
    fn into_result<R>(self) -> anyhow::Result<R>
    where
        R: DeserializeOwned,
        T: Into<Value>,
    {
        match self {
            Self::Success { result, .. } => {
                let value = result.into();
                serde_json::from_value::<R>(value)
                    .map_err(|e| anyhow!("Failed to deserialize result: {e}"))
            }
            Self::Error { error, .. } => {
                bail!("Websocket API error: {} - {}", error.code, error.msg)
            }
        }
    }
}

/// Structure for WebSocket error responses from the server.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct WebsocketError {
    pub code: i32,
    pub msg: String,
}

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

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PlaceOrderResponse {
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

#[skip_serializing_none]
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct QueryOrderRequest {
    pub symbol: String,
    pub order_id: Option<u64>,
    pub orig_client_order_id: Option<String>,
    pub recv_window: Option<u64>,
    pub api_key: Option<String>,
    pub timestamp: Option<u64>,
    pub signature: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct QueryOrderResponse {
    pub symbol: String,
    pub order_id: u64,
    pub order_list_id: i64,
    pub client_order_id: String,
    #[serde(with = "rust_decimal::serde::float")]
    pub price: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    pub orig_qty: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    pub executed_qty: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    pub cummulative_quote_qty: Decimal,
    pub status: OrderStatus,
    pub time_in_force: TimeInForce,
    #[serde(rename = "type")]
    pub order_type: OrderType,
    #[serde(rename = "side")]
    pub order_side: OrderSide,
    #[serde(with = "rust_decimal::serde::float")]
    pub stop_price: Decimal,
    pub time: u64,
    pub update_time: u64,
    pub is_working: bool,
    pub working_time: u64,
    #[serde(with = "rust_decimal::serde::float")]
    pub orig_quote_order_qty: Decimal,
}
