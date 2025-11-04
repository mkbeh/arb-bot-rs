use std::{
    fmt,
    sync::Arc,
    time::{Duration, SystemTime},
};

use anyhow::bail;
use futures_util::{
    Sink, SinkExt, StreamExt,
    stream::{SplitSink, SplitStream},
};
use rust_decimal::{Decimal, prelude::ToPrimitive};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use tokio::{
    net::TcpStream,
    sync::{Mutex, oneshot},
    time::interval,
};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async, tungstenite::Message};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error};
use url::Url;

use crate::libs::kucoin_api::utils;

type EventCallback<'a, T> = Box<dyn FnMut(T) -> anyhow::Result<()> + 'a + Send>;
type Writer = SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>;
type Reader = SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>;

pub struct WebsocketStream<'a, Event> {
    ws_url: String,
    ping_interval: Duration,
    writer: Option<Arc<Mutex<Writer>>>,
    reader: Option<Reader>,
    shutdown_tx: Option<oneshot::Sender<()>>,
    ping_handle: Option<tokio::task::JoinHandle<()>>,
    callback: Option<EventCallback<'a, Event>>,
}

impl<'a, Event: DeserializeOwned> WebsocketStream<'a, Event> {
    pub fn new(ws_url: String, ping_interval: u64) -> Self {
        Self {
            ws_url,
            ping_interval: Duration::from_millis(ping_interval),
            shutdown_tx: None,
            ping_handle: None,
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

    pub async fn connect(
        &mut self,
        topic: String,
        token: String,
        private_channel: bool,
    ) -> anyhow::Result<()> {
        let timestamp = utils::get_timestamp(SystemTime::now())?;
        let ws_url = format!("{}?token={}&connectId={}", self.ws_url, token, timestamp);
        let url = Url::parse(&ws_url)?;
        self.connect_ws(url).await?;

        let writer = Arc::clone(
            self
                .writer
                .as_ref()
                .expect("Writer must be set in connect_ws"),
        );

        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        self.shutdown_tx = Some(shutdown_tx);

        self.ping_handle = Some(tokio::spawn(ping_loop(
            writer,
            shutdown_rx,
            self.ping_interval,
        )));

        self.subscribe(timestamp, topic, private_channel).await
    }

    pub async fn disconnect(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }

        if let Some(writer) = self.writer.take() {
            let mut w = writer.lock().await;
            let _ = w.close().await;
        }

        self.writer = None;
        self.reader = None;
        self.ping_handle = None;
    }

    pub async fn subscribe(
        &mut self,
        ts: u64,
        topic: String,
        private_channel: bool,
    ) -> anyhow::Result<()> {
        let subscribe_msg = SubscribeMessage::new(topic, ts, private_channel);
        let json_msg = serde_json::to_string(&subscribe_msg)?;
        if let Some(ref writer) = self.writer {
            let mut w = writer.lock().await;
            w.send(Message::text(json_msg)).await?;
        } else {
            bail!("Writer not available for subscribe");
        }
        Ok(())
    }

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
                self.writer = Some(Arc::new(Mutex::new(writer)));
                self.reader = Some(reader);
                Ok(())
            }
            Err(e) => bail!("Received error during handshake: {}", e),
        }
    }

    fn is_connected(&self) -> bool {
        self.writer.is_some() && self.reader.is_some()
    }
}

async fn ping_loop<S>(
    writer: Arc<Mutex<S>>,
    mut shutdown_rx: oneshot::Receiver<()>,
    ping_interval: Duration,
) where
    S: SinkExt<Message> + Unpin,
    <S as Sink<Message>>::Error: fmt::Debug,
{
    let mut interval = interval(ping_interval);

    loop {
        tokio::select! {
            _ = &mut shutdown_rx => {
                let mut w = writer.lock().await;
                let _ = w.send(Message::Close(None)).await;
                break;
            }
            _ = interval.tick() => {
                let ts = utils::get_timestamp(SystemTime::now()).unwrap();
                match serde_json::to_string(&PingMessage::new(ts)) {
                    Ok(ping_msg) => {
                        let mut w = writer.lock().await;
                        if let Err(e) = w.send(Message::from(ping_msg)).await {
                            error!("Failed to send ping: {:?}", e);
                            break;
                        }
                    },
                    Err(e) => {
                        error!("Failed to serialize ping message: {:?}", e);
                        break;
                    }
                }
            }
        }
    }
}

/// # Arguments
///
/// * `symbol`: the market symbol
pub fn order_book_increment_topic(symbols: &[&str]) -> String {
    let s = symbols_to_comma_separated(symbols);
    format!("/market/level2:{}", s)
}

fn symbols_to_comma_separated(symbols: &[&str]) -> String {
    symbols
        .iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PingMessage {
    id: u64,
    #[serde(rename = "type")]
    event_type: String,
}

impl PingMessage {
    fn new(ts: u64) -> Self {
        Self {
            id: ts,
            event_type: "ping".to_string(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SubscribeMessage {
    id: u64,
    #[serde(rename = "type")]
    event_type: String,
    topic: String,
    private_channel: bool,
    response: bool,
}

impl SubscribeMessage {
    fn new(topic: String, ts: u64, private_channel: bool) -> Self {
        Self {
            id: ts,
            event_type: "subscribe".to_string(),
            response: true,
            private_channel,
            topic,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Events {
    #[serde(alias = "welcome")]
    Welcome { id: Option<String> },

    #[serde(alias = "ack")]
    Ack { id: Option<String> },

    #[serde(alias = "pong")]
    Pong { id: Option<String> },

    #[serde(alias = "error")]
    Error(Box<WebsocketError>),

    #[serde(alias = "message")]
    Message(Box<MessageEvents>),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "subject", content = "data", rename_all = "lowercase")]
pub enum MessageEvents {
    #[serde(alias = "trade.l2update")]
    IncrementOrderBook(Box<Level2Update>),
}

#[derive(Debug, Clone, Deserialize)]
pub struct WebsocketError {
    pub id: String,
    pub code: Option<i32>,
    #[serde(rename = "type")]
    pub event_type: String,
    pub data: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Level2Update {
    pub symbol: String,
    pub time: u64,
    pub sequence_start: u64,
    pub sequence_end: u64,
    pub changes: Changes,
}

impl Level2Update {
    /// Returns the most recent ask (price, size, seq) by max sequence, or None if asks is empty.
    pub fn latest_ask(&self) -> Option<(Decimal, Decimal, u64)> {
        self.changes
            .asks
            .iter()
            .max_by_key(|row| row.2.to_u64().unwrap_or(0u64)) // Max по seq (0 если None)
            .and_then(|row| {
                // Фильтруем: только если to_u64() succeeds
                row.2.to_u64().map(|seq| (row.0, row.1, seq)) // map: Option<u64> → Option<(D,D,u64)>
            })
    }

    /// Returns the most recent bid (price, size, seq) in the max sequence, or None if bids is
    /// empty.
    pub fn latest_bid(&self) -> Option<(Decimal, Decimal, u64)> {
        self.changes
            .bids
            .iter()
            .max_by_key(|row| row.2.to_u64().unwrap_or(0u64))
            .and_then(|row| row.2.to_u64().map(|seq| (row.0, row.1, seq)))
    }

    /// Returns latest asks and bids.
    pub fn latest_top(
        &self,
    ) -> (
        Option<(Decimal, Decimal, u64)>,
        Option<(Decimal, Decimal, u64)>,
    ) {
        (self.latest_ask(), self.latest_bid())
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Changes {
    pub asks: Vec<(Decimal, Decimal, Decimal)>, // price/size/sequence
    pub bids: Vec<(Decimal, Decimal, Decimal)>,
}
