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
use rust_decimal::Decimal;
use serde::{
    Deserialize, Deserializer, Serialize, de,
    de::{DeserializeOwned, SeqAccess},
};
use serde_json::json;
use tokio::{
    net::TcpStream,
    sync::{Mutex, oneshot},
    time::interval,
};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async, tungstenite::Message};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error};
use url::Url;

use crate::libs::kucoin_api::{
    enums::{FeeType, Liquidity, OrderChangeType, OrderSide, OrderStatus, OrderType},
    utils::get_timestamp,
};

type EventCallback<'a, T> = Box<dyn FnMut(T) -> anyhow::Result<()> + 'a + Send>;
pub(crate) type Writer = SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>;
pub(crate) type Reader = SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>;

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

    pub async fn connect(&mut self, topics: &[Topic], token: String) -> anyhow::Result<()> {
        let timestamp = get_timestamp(SystemTime::now())?;
        let ws_url = format!("{}?token={}&connectId={}", self.ws_url, token, timestamp);
        let url = Url::parse(&ws_url)?;
        self.connect_ws(url).await?;

        let writer = Arc::clone(
            self.writer
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

        for topic in topics {
            if let Err(e) = self.subscribe(timestamp, topic).await {
                bail!(e);
            }
        }

        Ok(())
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

    pub async fn subscribe(&mut self, ts: u64, topic: &Topic) -> anyhow::Result<()> {
        let subscribe_msg = SubscribeMessage::new(topic.stream.clone(), ts, topic.private);
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

pub async fn ping_loop<S>(
    writer: Arc<Mutex<S>>,
    mut shutdown_rx: oneshot::Receiver<()>,
    ping_interval: Duration,
) where
    S: SinkExt<Message> + Unpin,
    <S as Sink<Message>>::Error: fmt::Debug,
{
    let mut ping_timer = interval(ping_interval);
    ping_timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            _ = &mut shutdown_rx => {
                let _ = writer.lock().await.send(Message::Close(None)).await;
                break;
            }
            _ = ping_timer.tick() => {
                match ping_message() {
                    Ok(payload) => {
                        if let Err(e) = writer.lock().await.send(Message::Text(payload.into())).await {
                            error!("Failed to send ping: {:?}", e);
                            break;
                        }
                    }
                    Err(e) => {
                        error!("Failed to generate ping message: {:?}", e);
                        break;
                    }
                }
            }
        }
    }
}

fn ping_message() -> anyhow::Result<String> {
    let timestamp = get_timestamp(SystemTime::now())?;
    let ping = json!({
        "id": timestamp,
        "op": "ping"
    });
    Ok(ping.to_string())
}

/// # Arguments
///
/// * `symbol`: the market symbol
pub fn order_book_increment_topic(symbols: &[&str]) -> Topic {
    let s = symbols_to_comma_separated(symbols);
    Topic {
        stream: format!("/market/level2:{}", s),
        private: false,
    }
}

pub fn order_change_topic() -> Topic {
    Topic {
        stream: String::from("/spotMarket/tradeOrdersV2"),
        private: true,
    }
}

pub fn account_balance_topic() -> Topic {
    Topic {
        stream: String::from("/account/balance"),
        private: true,
    }
}

fn symbols_to_comma_separated(symbols: &[&str]) -> String {
    symbols
        .iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

pub struct Topic {
    stream: String,
    private: bool,
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
    #[serde(alias = "orderChange")]
    OrderChange(Box<OrderChange>),
    #[serde(alias = "account.balance")]
    AccountBalance(Box<AccountBalance>),
}

#[derive(Debug, Clone, Deserialize)]
pub struct WebsocketError {
    pub id: String,
    pub code: Option<i32>,
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
    #[inline]
    fn latest_row(rows: &[OrderRow]) -> Option<OrderRow> {
        rows.iter().max_by_key(|row| row.2).cloned()
    }

    #[inline]
    pub fn latest_ask(&self) -> Option<OrderRow> {
        Self::latest_row(&self.changes.asks)
    }

    #[inline]
    pub fn latest_bid(&self) -> Option<OrderRow> {
        Self::latest_row(&self.changes.bids)
    }

    #[inline]
    pub fn latest_top(&self) -> (Option<OrderRow>, Option<OrderRow>) {
        (self.latest_ask(), self.latest_bid())
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Changes {
    pub asks: Vec<OrderRow>,
    pub bids: Vec<OrderRow>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OrderRow(pub Decimal, pub Decimal, pub u64); // price, size, sequence

impl<'de> Deserialize<'de> for OrderRow {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct RowVisitor;

        impl<'de> de::Visitor<'de> for RowVisitor {
            type Value = OrderRow;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("array of 3 strings: [price str, size str, sequence str]")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let price_str: String = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(0, &self))?;
                let size_str: String = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(1, &self))?;
                let seq_str: String = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(2, &self))?;

                let price = price_str.parse::<Decimal>().map_err(de::Error::custom)?;
                let size = size_str.parse::<Decimal>().map_err(de::Error::custom)?;
                let sequence = seq_str.parse::<u64>().map_err(de::Error::custom)?;

                // Проверяем, что больше элементов нет
                if seq.next_element::<String>()?.is_some() {
                    return Err(de::Error::invalid_length(3, &self));
                }

                Ok(OrderRow(price, size, sequence))
            }
        }

        deserializer.deserialize_tuple(3, RowVisitor)
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderChange {
    pub status: OrderStatus,
    #[serde(rename = "type")]
    pub order_change_type: OrderChangeType,
    pub symbol: String,
    pub side: OrderSide,
    pub order_type: OrderType,
    pub fee_type: Option<FeeType>,
    pub liquidity: Option<Liquidity>,
    #[serde(default, with = "rust_decimal::serde::float_option")]
    pub price: Option<Decimal>,
    pub order_id: String,
    pub client_oid: String,
    pub trade_id: Option<String>,
    #[serde(default, with = "rust_decimal::serde::float_option")]
    pub origin_size: Option<Decimal>, // limit/market sell
    #[serde(default, with = "rust_decimal::serde::float_option")]
    pub origin_funds: Option<Decimal>, // market buy/sell
    #[serde(default, with = "rust_decimal::serde::float_option")]
    pub size: Option<Decimal>,
    #[serde(default, with = "rust_decimal::serde::float_option")]
    pub filled_size: Option<Decimal>,
    #[serde(default, with = "rust_decimal::serde::float_option")]
    pub match_size: Option<Decimal>,
    #[serde(default, with = "rust_decimal::serde::float_option")]
    pub match_price: Option<Decimal>,
    #[serde(default, with = "rust_decimal::serde::float_option")]
    pub canceled_size: Option<Decimal>,
    #[serde(default, with = "rust_decimal::serde::float_option")]
    pub old_size: Option<Decimal>,
    #[serde(default, with = "rust_decimal::serde::float_option")]
    pub remain_size: Option<Decimal>,
    #[serde(default, with = "rust_decimal::serde::float_option")]
    pub remain_funds: Option<Decimal>,
    pub pt: Option<u64>,
    pub ts: u64,
    pub order_time: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountBalance {
    pub account_id: String,
    pub currency: String,
    #[serde(default, with = "rust_decimal::serde::float")]
    pub total: Decimal,
    #[serde(default, with = "rust_decimal::serde::float")]
    pub available: Decimal,
    #[serde(default, with = "rust_decimal::serde::float")]
    pub hold: Decimal,
    #[serde(default, with = "rust_decimal::serde::float")]
    pub available_change: Decimal,
    #[serde(default, with = "rust_decimal::serde::float")]
    pub hold_change: Decimal,
    pub relation_context: Option<RelationContext>,
    pub relation_event: String,
    pub relation_event_id: String,
    pub time: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelationContext {
    pub symbol: String,
    pub order_id: String,
}

#[cfg(test)]
mod tests {
    use crate::libs::kucoin_api::stream::OrderChange;

    #[test]
    fn test_deserialize_order_change_received() {
        let data = r#"
        {
           "clientOid":"26d2ebe0-691d-463b-8258-5862b28c050b",
           "orderId":"69137565e554ab000700f8ae",
           "orderTime":1762882917049,
           "orderType":"market",
           "originFunds":"15",
           "pt":1762882917070,
           "side":"buy",
           "status":"new",
           "symbol":"BTC-USDT",
           "ts":1762882917069000000,
           "type":"received"
        }
        "#;
        serde_json::from_str::<OrderChange>(data).unwrap();
    }

    #[test]
    fn test_deserialize_order_change_match() {
        let data = r#"
        {
           "canceledSize":"0",
           "clientOid":"5c52e11203aa677f33e493fc",
           "feeType":"takerFee",
           "filledSize":"0.00001",
           "liquidity":"taker",
           "matchPrice":"71171.9",
           "matchSize":"0.00001",
           "orderId":"6720da3fa30a360007f5f832",
           "orderTime":1730206271588,
           "orderType":"market",
           "originSize":"0.00001",
           "remainSize":"0",
           "side":"buy",
           "size":"0.00001",
           "status":"match",
           "symbol":"BTC-USDT",
           "tradeId":"11116472408358913",
           "ts":1730206271616000000,
           "type":"match"
        }
        "#;
        serde_json::from_str::<OrderChange>(data).unwrap();
    }

    #[test]
    fn test_deserialize_order_change_canceled() {
        let data = r#"
        {
           "canceledSize":"0.00002",
           "clientOid":"5c52e11203aa677f33e493fb",
           "filledSize":"0",
           "orderId":"6720df7640e6fe0007b57696",
           "orderTime":1730207606848,
           "orderType":"limit",
           "originSize":"0.00002",
           "price":"50000",
           "remainFunds":"0",
           "remainSize":"0",
           "side":"buy",
           "size":"0.00001",
           "status":"done",
           "symbol":"BTC-USDT",
           "ts":1730207624559000000,
           "type":"canceled"
        }
        "#;
        serde_json::from_str::<OrderChange>(data).unwrap();
    }
}
