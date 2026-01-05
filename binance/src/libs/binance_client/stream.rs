//! WebSocket stream module for handling real-time data feeds.
//!
//! This module provides a generic `WebsocketStream` struct for connecting to WebSocket endpoints,
//! managing bidirectional communication, and dispatching deserialized events via callbacks.
//! It is designed for exchange APIs (e.g., Binance)
//!
//! # Usage
//!
//! ```rust,no_run
//! use anyhow::Result;
//! use serde::Deserialize;
//! use tokio_util::sync::CancellationToken;
//!
//! // Define your event type (must implement DeserializeOwned).
//! #[derive(Debug, Deserialize)]
//! struct MyEvent {/* fields */}
//!
//! #[tokio::main]
//! async fn main() -> Result<()> {
//!     use binance::libs::binance_client::stream::WebsocketStream;
//!
//!     let mut ws = WebsocketStream::new("wss://stream.example.com/ws".to_string()).with_callback(
//!         |event: MyEvent| {
//!             println!("Event: {:?}", event);
//!             Ok(())
//!         },
//!     );
//!
//!     // Connect to a stream.
//!     ws.connect("btcusdt@bookTicker".to_string()).await?;
//!
//!     // Handle messages until cancelled.
//!     let token = CancellationToken::new();
//!     ws.handle_messages(token.clone()).await?;
//!
//!     // In another task, cancel if needed.
//!     // token.cancel();
//!     Ok(())
//! }
//! ```

use anyhow::bail;
use futures_util::{
    SinkExt, StreamExt,
    stream::{SplitSink, SplitStream},
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use tokio::net::TcpStream;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async, tungstenite::Message};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error};
use url::Url;

use crate::libs::binance_client;

/// Prefix for multi-stream WebSocket URLs.
static STREAM_PREFIX: &str = "stream";

/// Prefix for single-stream WebSocket URLs.
static WS_PREFIX: &str = "ws";

/// Type alias for an event callback function.
type EventCallback<'a, T> = Box<dyn FnMut(T) -> anyhow::Result<()> + 'a + Send>;

/// Type alias for the WebSocket writer sink.
type Writer = SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>;

/// Type alias for the WebSocket reader stream.
type Reader = SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>;

/// Generic WebSocket stream handler for real-time event processing.
pub struct WebsocketStream<'a, Event> {
    ws_url: String,
    writer: Option<Writer>,
    reader: Option<Reader>,
    callback: Option<EventCallback<'a, Event>>,
}

impl<'a, Event: DeserializeOwned> WebsocketStream<'a, Event> {
    #[must_use]
    pub fn new(ws_url: String) -> Self {
        Self {
            ws_url,
            writer: None,
            reader: None,
            callback: None,
        }
    }

    /// Sets a callback to handle incoming deserialized events.
    ///
    /// The callback is invoked for each valid text message after deserialization.
    #[must_use]
    pub fn with_callback<Callback>(mut self, callback: Callback) -> Self
    where
        Callback: FnMut(Event) -> anyhow::Result<()> + 'a + Send,
    {
        self.callback = Some(Box::new(callback));
        self
    }

    /// Connects to a single stream endpoint.
    pub async fn connect(&mut self, stream: String) -> anyhow::Result<()> {
        let s = format!("{}/{WS_PREFIX}/{stream}", self.ws_url);
        let url = Url::parse(s.as_str())?;
        self.connect_ws(url).await
    }

    /// Connects to multiple streams in a single WebSocket connection.
    pub async fn connect_multiple(&mut self, streams: &[String]) -> anyhow::Result<()> {
        let s = format!("{}/{STREAM_PREFIX}", self.ws_url);
        let mut url = Url::parse(s.as_str())?;
        url.query_pairs_mut()
            .append_pair("streams", streams.join("/").as_str());
        self.connect_ws(url).await
    }

    /// Disconnects the WebSocket stream gracefully.
    ///
    /// Sends a close message if connected and clears the reader/writer.
    pub async fn disconnect(&mut self) {
        if let Some(ref mut writer) = self.writer {
            let _ = writer.close().await;
        }

        self.writer = None;
        self.reader = None;
    }

    /// Handles incoming messages in a loop until cancellation or closure.
    pub async fn handle_messages(&mut self, token: CancellationToken) -> anyhow::Result<()> {
        if !self.is_connected() {
            bail!("Websocket stream is not connected");
        }

        let reader = self.reader.as_mut().unwrap();

        loop {
            tokio::select! {
                _ = token.cancelled() => {
                    break;
                }
                Some(result) = reader.next() => {
                    match result {
                        Ok(Message::Text(message)) => {
                            Self::handle_text_message(&mut self.callback, &message)?
                        }
                        Ok(Message::Ping(data)) => {
                            if let Some(ref mut writer) = self.writer
                                && let Err(e) = writer.send(Message::Pong(data)).await {
                                    error!("Failed to send pong: {:?}", e);
                                }
                        }
                        Ok(Message::Close(_)) => {
                            debug!("Websocket stream closed");
                            break;
                        }
                        Err(e) => {
                            error!("Websocket stream error: {:?}", e.to_string());
                            break;
                        }
                        _ => {}
                    }
                }
            }
        }

        Ok(())
    }

    /// Deserializes a text message and invokes the callback if present.
    fn handle_text_message(
        callback: &mut Option<EventCallback<'a, Event>>,
        text: &str,
    ) -> anyhow::Result<()> {
        if let Some(callback) = callback {
            match serde_json::from_str::<Event>(text) {
                Ok(event) => {
                    if let Err(e) = callback(event) {
                        bail!("Failed to call callback: {e} - {text:?}");
                    };
                }
                Err(e) => {
                    bail!("Failed to parse websocket event: {e} - {text:?}");
                }
            }
        };
        Ok(())
    }

    /// Performs the WebSocket handshake and splits the stream.
    async fn connect_ws(&mut self, url: Url) -> anyhow::Result<()> {
        match connect_async(url.as_str()).await {
            Ok((stream, _)) => {
                let (writer, reader) = stream.split();
                self.writer = Some(writer);
                self.reader = Some(reader);
                Ok(())
            }
            Err(e) => bail!("Received error during handshake: {e}"),
        }
    }

    /// Checks if the WebSocket is currently connected.
    ///
    /// # Returns
    ///
    /// `true` if both reader and writer are present.
    pub fn is_connected(&self) -> bool {
        self.writer.is_some() && self.reader.is_some()
    }
}

/// # Arguments
///
/// * `symbol`: the market symbol
#[must_use]
pub fn book_ticker_stream(symbol: &str) -> String {
    format!("{symbol}@bookTicker")
}

/// # Arguments
///
/// * `symbol`: the market symbol
/// * `levels`: 5, 10 or 20
/// * `update_speed`: 1000 or 100
#[must_use]
pub fn partial_book_depth_stream(symbol: &str, levels: u16, update_speed: u16) -> String {
    format!("{symbol}@depth{levels}@{update_speed}ms")
}

/// Wrapper for stream events containing metadata and payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamEvent<T> {
    stream: String,
    pub data: T,
}

/// Enum representing possible deserialized event types from the stream.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Events {
    BookTicker(Box<BookTickerEvent>),
    PartialBookDepth(Box<OrderBook>),
}

/// Event structure for book ticker updates (best bid/ask prices and quantities).
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BookTickerEvent {
    #[serde(rename = "u")]
    pub update_id: u64,

    #[serde(rename = "s")]
    pub symbol: String,

    #[serde(rename = "b")]
    #[serde(with = "rust_decimal::serde::float")]
    pub best_bid_price: Decimal,

    #[serde(rename = "B")]
    #[serde(with = "rust_decimal::serde::float")]
    pub best_bid_qty: Decimal,

    #[serde(rename = "a")]
    #[serde(with = "rust_decimal::serde::float")]
    pub best_ask_price: Decimal,

    #[serde(rename = "A")]
    #[serde(with = "rust_decimal::serde::float")]
    pub best_ask_qty: Decimal,
}

/// Event structure for partial order book depth updates.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct OrderBook {
    pub last_update_id: u64,
    #[serde(rename = "bids")]
    pub bids: Vec<binance_client::OrderBookUnit>,
    #[serde(rename = "asks")]
    pub asks: Vec<binance_client::OrderBookUnit>,
}

/// Unit structure for order book entries (price and quantity).
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct OrderBookUnit {
    #[serde(with = "rust_decimal::serde::float")]
    pub price: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    pub qty: Decimal,
}
