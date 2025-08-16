use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::bail;
use futures_util::{SinkExt, StreamExt};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use tokio::net::TcpStream;
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, connect_async,
    tungstenite::{Message, handshake::client::Response},
};
use tracing::{error, info};
use url::Url;

static STREAM_PREFIX: &str = "stream";
static WS_PREFIX: &str = "ws";

pub struct WebsocketStream<'a, WebsocketEvent> {
    ws_url: String,
    socket: Option<(WebSocketStream<MaybeTlsStream<TcpStream>>, Response)>,
    cb: Option<Box<dyn FnMut(WebsocketEvent) -> anyhow::Result<()> + 'a + Send>>,
}

impl<'a, WebsocketEvent: DeserializeOwned> WebsocketStream<'a, WebsocketEvent> {
    pub fn new(ws_url: String) -> Self {
        Self {
            ws_url,
            socket: None,
            cb: None,
        }
    }

    pub fn with_callback<Callback>(mut self, cb: Callback) -> Self
    where
        Callback: FnMut(WebsocketEvent) -> anyhow::Result<()> + 'a + Send,
    {
        self.cb = Some(Box::new(cb));
        self
    }

    pub async fn connect(&mut self, stream: String) -> anyhow::Result<()> {
        let s = format!("{}/{}/{}", self.ws_url, WS_PREFIX, stream);
        let url = Url::parse(s.as_str())?;
        self.connect_ws(url).await
    }

    pub async fn connect_multiple(&mut self, streams: &[String]) -> anyhow::Result<()> {
        let s = format!("{}/{}", self.ws_url, STREAM_PREFIX);
        let mut url = Url::parse(s.as_str())?;
        url.query_pairs_mut()
            .append_pair("streams", streams.join("/").as_str());
        self.connect_ws(url).await
    }

    pub async fn disconnect(&mut self) -> anyhow::Result<()> {
        if let Some(ref mut stream) = self.socket {
            stream.0.close(None).await?;
            Ok(())
        } else {
            bail!("Failed to disconnect websocket connection");
        }
    }

    pub async fn run(&mut self, flag: &AtomicBool) -> anyhow::Result<()> {
        if let Some((ref mut ws_stream, _)) = self.socket {
            let (mut write, mut read) = ws_stream.split();

            while flag.load(Ordering::Relaxed) {
                if let Some(msg) = read.next().await {
                    match msg {
                        Ok(Message::Text(msg)) => {
                            if msg.is_empty() {
                                continue;
                            }

                            if let Some(cb) = self.cb.as_mut() {
                                let event = serde_json::from_str(msg.as_str())?;
                                cb(event)?;
                            }
                        }
                        Ok(Message::Ping(ping_data)) => {
                            if let Err(e) = write.send(Message::Pong(ping_data)).await {
                                error!("Failed to send pong: {:?}", e);
                                break;
                            }
                        }
                        Ok(Message::Close(_)) => {
                            info!("Websocket stream closed");
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

    async fn connect_ws(&mut self, url: Url) -> anyhow::Result<()> {
        match connect_async(url.as_str()).await {
            Ok(stream) => {
                self.socket = Some(stream);
                Ok(())
            }
            Err(e) => bail!("Received error during handshake: {}", e),
        }
    }
}

/// # Arguments
///
/// * `symbol`: the market symbol
pub fn book_ticker_stream(symbol: &str) -> String {
    format!("{symbol}@bookTicker")
}

/// # Arguments
///
/// * `symbol`: the market symbol
/// * `levels`: 5, 10 or 20
/// * `update_speed`: 1000 or 100
pub fn partial_book_depth_stream(symbol: &str, levels: u16, update_speed: u16) -> String {
    format!("{symbol}@depth{levels}@{update_speed}ms")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamEvent<T> {
    stream: String,
    pub data: T,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Events {
    BookTicker(BookTickerEvent),
    PartialBookDepth(OrderBook),
}

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

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct OrderBook {
    pub last_update_id: u64,
    #[serde(rename = "bids")]
    pub bids: Vec<crate::libs::binance_api::OrderBookUnit>,
    #[serde(rename = "asks")]
    pub asks: Vec<crate::libs::binance_api::OrderBookUnit>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct OrderBookUnit {
    #[serde(with = "rust_decimal::serde::float")]
    pub price: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    pub qty: Decimal,
}
