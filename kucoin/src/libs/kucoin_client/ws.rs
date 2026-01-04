//! KuCoin WebSocket client for authenticated private trading operations.
//!
//! # Usage
//!
//! ```rust,no_run
//! use anyhow::Result;
//! use kucoin_bot::libs::kucoin_client::ws::{
//!     AddOrderRequest, ConnectConfig, WebsocketClient, connect_ws,
//! };
//! use tokio_util::sync::CancellationToken;
//!
//! #[tokio::main]
//! async fn main() -> Result<()> {
//!     let config = ConnectConfig {
//!         ws_url: "wss://ws-api.kucoin.com/v1/private".to_string(),
//!         token: "your-api-key".to_string(),
//!         secret_key: "your-secret-key".to_string(),
//!         passphrase: "your-passphrase".to_string(),
//!     };
//!
//!     let token = CancellationToken::new();
//!     let mut client = connect_ws(config, token.clone()).await?;
//!
//!     // Disconnect.
//!     client.disconnect().await;
//!     Ok(())
//! }
//! ```

use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, SystemTime},
};

use anyhow::{Context, anyhow, bail};
use futures_util::{SinkExt, StreamExt};
use percent_encoding::{NON_ALPHANUMERIC, percent_encode};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;
use serde_with::skip_serializing_none;
use tokio::sync::{Mutex, mpsc, oneshot};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error};
use url::Url;
use uuid::Uuid;

use crate::libs::kucoin_client::{
    enums::{OrderSide, OrderType},
    stream::{Reader, Writer, ping_loop},
    utils,
    utils::sign,
};

/// Type for tracking pending requests with response channels.
type PendingRequests = HashMap<String, mpsc::Sender<anyhow::Result<WebsocketResponse<Value>>>>;

/// Configuration for establishing a WebSocket connection to KuCoin API.
#[derive(Clone)]
pub struct ConnectConfig {
    pub ws_url: String,
    pub token: String,
    pub secret_key: String,
    pub passphrase: String,
}

/// Main WebSocket client structure for private trading operations.
pub struct WebsocketClient {
    writer: Arc<Mutex<Writer>>,
    pending_requests: Arc<Mutex<PendingRequests>>,
    shutdown_tx: Option<oneshot::Sender<()>>,
    ping_handle: Option<tokio::task::JoinHandle<()>>,
    reader_handle: Option<tokio::task::JoinHandle<()>>,
    response_timeout: Duration,
}

/// Establishes a WebSocket connection to KuCoin and initializes the client.
pub async fn connect_ws(
    conf: ConnectConfig,
    cancellation_token: CancellationToken,
) -> anyhow::Result<WebsocketClient> {
    let ws_url = build_ws_url(&conf)?;
    let (ws_stream, _) = connect_async(ws_url.as_str()).await?;

    let (mut writer, mut reader) = ws_stream.split();

    // Handle authentication handshake
    let auth_msg = reader
        .next()
        .await
        .context("No authentication message received")?
        .context("Authentication message error")?;

    if let Message::Text(auth_response) = auth_msg {
        let session_info = sign(&auth_response, &conf.secret_key);
        writer
            .send(Message::Text(session_info.into()))
            .await
            .context("Failed to send session signature")?;
    } else {
        bail!("Unexpected authentication message type: {auth_msg:?}");
    }

    // Handle welcome message and extract ping interval
    let welcome_msg = reader
        .next()
        .await
        .context("No welcome message received")?
        .context("Welcome message error")?;

    let ping_interval = if let Message::Text(welcome_json) = welcome_msg {
        // Parse ping_interval from JSON (default to 18s if missing)
        let welcome: serde_json::Value =
            serde_json::from_str(&welcome_json).context("Failed to parse welcome JSON")?;
        Duration::from_millis(welcome["pingInterval"].as_u64().unwrap_or(18_000))
    } else {
        bail!("Unexpected welcome message type: {welcome_msg:?}");
    };

    let writer = Arc::new(Mutex::new(writer));
    let pending_requests = Arc::new(Mutex::new(HashMap::new()));

    // Spawn reader task for concurrent handling
    let reader_handle = tokio::spawn(reader_handle(
        reader,
        pending_requests.clone(),
        cancellation_token.clone(),
    ));

    // Setup shutdown and ping loop
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let ping_handle = tokio::spawn(ping_loop(writer.clone(), shutdown_rx, ping_interval));

    Ok(WebsocketClient {
        writer,
        pending_requests,
        shutdown_tx: Some(shutdown_tx),
        ping_handle: Some(ping_handle),
        reader_handle: Some(reader_handle),
        response_timeout: Duration::from_secs(10),
    })
}

impl WebsocketClient {
    /// Adds a new order via the KuCoin WebSocket API.
    ///
    /// Constructs and sends the signed request, awaits the response.
    /// Supports market/limit orders with size or funds.
    ///
    /// # Arguments
    ///
    /// * `request` - Order addition parameters.
    ///
    /// # Returns
    ///
    /// `AddOrderResponse` on success.
    ///
    /// # Errors
    ///
    /// - Signature generation failures.
    /// - Request serialization or transmission errors.
    /// - Timeout or no response from server.
    /// - API errors (e.g., invalid parameters).
    pub async fn add_order(
        &mut self,
        request: AddOrderRequest,
    ) -> anyhow::Result<AddOrderResponse> {
        self.send_request::<AddOrderRequest, AddOrderResponse>(WebsocketApi::AddOrder, request)
            .await
    }

    /// Internal method to send a generic signed request and await response.
    ///
    /// Serializes the request, tracks it via channel, sends over WebSocket,
    /// and waits for correlated response with timeout.
    ///
    /// # Type Parameters
    ///
    /// * `T` - The request parameter type (must implement `Serialize`).
    /// * `R` - The response type (must implement `DeserializeOwned`).
    ///
    /// # Arguments
    ///
    /// * `method` - The WebSocket API method (e.g., `WebsocketApi::AddOrder`).
    /// * `params` - The request parameters.
    ///
    /// # Returns
    ///
    /// A deserialized `R` on success.
    ///
    /// # Errors
    ///
    /// - Serialization failures.
    /// - WebSocket send errors.
    /// - Timeout, no response, or remote API errors.
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
            Ok(Some(Ok(response))) => response.into_result(),
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

    /// Disconnects the WebSocket connection gracefully.
    pub async fn disconnect(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }

        if let Some(handle) = self.ping_handle.take() {
            handle.abort();
        }

        if let Some(handle) = self.reader_handle.take() {
            handle.abort();
        }

        let mut w = self.writer.lock().await;
        let _ = w.send(Message::Close(None)).await;
    }
}

/// Background task for handling incoming messages from the reader.
async fn reader_handle(
    mut reader: Reader,
    pending: Arc<Mutex<PendingRequests>>,
    cancellation: CancellationToken,
) {
    loop {
        tokio::select! {
            _ = cancellation.cancelled() => {
                break;
            }
            msg = reader.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        if let Err(e) = handle_text_message(&text, &pending).await {
                            error!("Failed to handle message: {}", e);
                        }
                    }
                    Some(Ok(Message::Close(frame))) => {
                        debug!("Websocket closed: {:?}", frame);
                        notify_pending(&pending, anyhow!("Websocket closed")).await;
                        break;
                    }
                    Some(Err(e)) => {
                        error!("WS read error: {}", e);
                        notify_pending(&pending, anyhow!("WS read error: {e}")).await;
                        break;
                    }
                    None => {
                        debug!("WS stream ended");
                        break;
                    }
                    _ => {}
                }
            }
        }
    }
}

/// Processes a text message: deserializes as WebsocketEvent, routes responses.
async fn handle_text_message(
    text: &str,
    pending: &Arc<Mutex<PendingRequests>>,
) -> anyhow::Result<()> {
    let event: WebsocketEvent = serde_json::from_str(text).context("Parse websocket event")?;

    match event {
        WebsocketEvent::Pong(_) => {}
        WebsocketEvent::Response(response) => {
            let id = match &response {
                WebsocketResponse::Success(s) => s.id.clone(),
                WebsocketResponse::Error(e) => e.id.clone().unwrap_or_default(), // If error has id
            };

            if id.is_empty() {
                debug!("Response without id: {}", text);
                return Ok(());
            }

            let mut pending = pending.lock().await;
            if let Some(sender) = pending.remove(&id) {
                match response {
                    WebsocketResponse::Success(_) => {
                        let _ = sender.send(Ok(response)).await;
                    }
                    WebsocketResponse::Error(e) => {
                        return Err(anyhow!("Server error: code={}, msg={}", e.code, e.msg));
                    }
                };
            } else {
                debug!("No pending request for id: {}", id);
            }
        }
    }

    Ok(())
}

/// Notifies all pending requests of an error (e.g., on disconnect).
async fn notify_pending(pending: &Arc<Mutex<PendingRequests>>, err: anyhow::Error) {
    let mut pending = pending.lock().await;
    for (_, sender) in pending.drain() {
        let _ = sender.send(Err(anyhow!("Connection error: {err}"))).await;
    }
}

/// Builds the WebSocket URL with KuCoin authentication.
fn build_ws_url(conf: &ConnectConfig) -> anyhow::Result<Url> {
    let timestamp = utils::get_timestamp(SystemTime::now())?;
    let url_path = format!("apikey={}&timestamp={timestamp}", conf.token);
    let original = format!("{}{timestamp}", conf.token);
    let signature = sign(&original, &conf.secret_key);
    let sign_value = percent_encode(signature.as_bytes(), NON_ALPHANUMERIC).to_string();
    let passphrase_sign = percent_encode(
        sign(&conf.passphrase, &conf.secret_key).as_bytes(),
        NON_ALPHANUMERIC,
    )
    .to_string();

    let ws_url = format!(
        "{}/v1/private?{url_path}&sign={sign_value}&passphrase={passphrase_sign}",
        conf.ws_url
    );

    Url::parse(&ws_url).context("Invalid websocket url")
}

/// Enum representing supported WebSocket API methods.
///
/// Defines the private trading endpoints used in this client.
pub enum WebsocketApi {
    AddOrder,
}

impl From<WebsocketApi> for String {
    fn from(api: WebsocketApi) -> Self {
        Self::from(match api {
            WebsocketApi::AddOrder => "spot.order",
        })
    }
}

/// Internal request structure sent over WebSocket.
#[skip_serializing_none]
#[derive(Debug, Clone, Serialize)]
struct WebsocketRequest<T>
where
    T: Serialize,
{
    id: String,
    op: String,
    args: T,
}

impl<T> WebsocketRequest<T>
where
    T: Serialize,
{
    fn new(op: WebsocketApi, args: T) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            op: op.into(),
            args,
        }
    }
}

/// Enum for deserialized WebSocket events (pong or response).
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(untagged)]
pub enum WebsocketEvent {
    Pong(PongResponse),
    Response(WebsocketResponse<Value>),
}

/// Enum for API responses.
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(untagged)]
pub enum WebsocketResponse<T> {
    Success(SuccessResponse<T>),
    Error(ErrorResponse),
}

impl<T> WebsocketResponse<T> {
    fn into_result<R>(self) -> anyhow::Result<R>
    where
        R: DeserializeOwned,
        T: Into<Value>,
    {
        match self {
            Self::Success(v) => {
                let value = v.data.into();
                serde_json::from_value::<R>(value)
                    .map_err(|e| anyhow!("Failed to deserialize result: {e}"))
            }
            Self::Error(v) => {
                bail!("Server error: code={}, msg={}", v.code, v.msg)
            }
        }
    }
}

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

/// Structure for pong responses (ignored in handling).
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PongResponse {
    pub id: u64,
    pub op: String, // "pong"
    pub timestamp: u64,
}

/// Structure for successful API responses.
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SuccessResponse<T> {
    pub id: String,
    pub op: String,
    pub code: String,
    pub data: T,
    pub in_time: u64,
    pub out_time: u64,
}

/// Structure for error API responses.
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ErrorResponse {
    pub id: Option<String>,
    pub op: Option<String>,
    pub code: String,
    #[serde(rename = "type")]
    pub event_type: Option<String>,
    pub msg: String,
    pub in_time: u64,
    pub out_time: u64,
}

/// Request structure for adding an order.
#[skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddOrderRequest {
    pub client_oid: String,
    pub symbol: String,
    #[serde(rename = "type")]
    pub order_type: OrderType,
    #[serde(rename = "side")]
    pub order_side: OrderSide,
    pub size: Option<String>,
    pub funds: Option<String>,
}

/// Response structure for adding an order.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddOrderResponse {
    pub order_id: String,
    pub client_oid: String,
}
