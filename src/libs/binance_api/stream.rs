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

static STREAM_PREFIX: &str = "stream";
static WS_PREFIX: &str = "ws";

type EventCallback<'a, T> = Box<dyn FnMut(T) -> anyhow::Result<()> + 'a + Send>;
type Writer = SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>;
type Reader = SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>;

pub struct WebsocketStream<'a, Event> {
    ws_url: String,
    writer: Option<Writer>,
    reader: Option<Reader>,
    callback: Option<EventCallback<'a, Event>>,
}

impl<'a, Event: DeserializeOwned> WebsocketStream<'a, Event> {
    pub fn new(ws_url: String) -> Self {
        Self {
            ws_url,
            writer: None,
            reader: None,
            callback: None,
        }
    }

    pub fn with_callback<Callback>(mut self, callback: Callback) -> Self
    where
        Callback: FnMut(Event) -> anyhow::Result<()> + 'a + Send,
    {
        self.callback = Some(Box::new(callback));
        self
    }

    pub async fn connect(&mut self, stream: String) -> anyhow::Result<()> {
        let s = format!("{}/{WS_PREFIX}/{stream}", self.ws_url);
        let url = Url::parse(s.as_str())?;
        self.connect_ws(url).await
    }

    pub async fn connect_multiple(&mut self, streams: &[String]) -> anyhow::Result<()> {
        let s = format!("{}/{STREAM_PREFIX}", self.ws_url);
        let mut url = Url::parse(s.as_str())?;
        url.query_pairs_mut()
            .append_pair("streams", streams.join("/").as_str());
        self.connect_ws(url).await
    }

    pub async fn disconnect(&mut self) {
        if let Some(ref mut writer) = self.writer {
            let _ = writer.close().await;
        }

        self.writer = None;
        self.reader = None;
    }

    pub async fn handle_messages(&mut self, token: CancellationToken) -> anyhow::Result<()> {
        if !self.is_connected() {
            bail!("Websocket stream is not connected");
        }

        let writer = self.writer.as_mut().unwrap();
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
                            if let Err(e) = writer.send(Message::Pong(data)).await {
                                error!("Failed to send pong: {:?}", e);
                                break;
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

    fn handle_text_message(
        callback: &mut Option<EventCallback<'a, Event>>,
        text: &str,
    ) -> anyhow::Result<()> {
        if let Some(callback) = callback {
            match serde_json::from_str::<Event>(text) {
                Ok(event) => {
                    if let Err(e) = callback(event) {
                        bail!("Failed to call callback: {e} - {:?}", text);
                    };
                }
                Err(e) => {
                    bail!("Failed to parse websocket event: {e} - {:?}", text);
                }
            }
        };
        Ok(())
    }

    async fn connect_ws(&mut self, url: Url) -> anyhow::Result<()> {
        match connect_async(url.as_str()).await {
            Ok((stream, _)) => {
                let (writer, reader) = stream.split();
                self.writer = Some(writer);
                self.reader = Some(reader);
                Ok(())
            }
            Err(e) => bail!("Received error during handshake: {}", e),
        }
    }

    pub fn is_connected(&self) -> bool {
        self.writer.is_some() && self.reader.is_some()
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
    BookTicker(Box<BookTickerEvent>),
    PartialBookDepth(Box<OrderBook>),
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
