use std::{collections::HashMap, time::Duration};

use ahash::AHashMap;
use anyhow::{Context, bail};
use base64::{Engine, engine::general_purpose};
use futures_util::{
    SinkExt, StreamExt,
    stream::{SplitSink, SplitStream},
};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use simd_json::OwnedValue;
use solana_client::client_error::reqwest::Url;
use solana_sdk::pubkey::Pubkey;
use tokio::{
    net::TcpStream,
    time::{interval, sleep},
};
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, connect_async,
    tungstenite::{Error, Message, Utf8Bytes},
};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, warn};

use crate::libs::solana_client::{
    callback::BatchEventCallbackWrapper,
    models::{AccountEvent, Event, SlotEvent, SubscribeTarget, TxEvent},
    registry::{
        DEX_REGISTRY,
        traits::{DexParser, RegistryLookup},
    },
};

type StreamWriter = SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>;
type StreamReader = SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>;

#[derive(Clone, Debug)]
pub struct StreamConfig {
    /// The gRPC endpoint URL.
    pub endpoint: String,
    /// The interval in seconds at which WebSocket 'Ping' frames are sent
    /// to the server to keep the connection alive.
    pub ping_interval: Duration,
    /// Program IDs to subscribe to (via account_include).
    /// The maximum number of gRPC messages to
    /// accumulate in a single processing burst.
    pub batch_size: usize,
    /// The microsecond-grade duration to wait for additional
    /// messages after the first one arrives in the stream.
    pub batch_fill_timeout: Duration,
    /// List of Dex's programs ids.
    pub program_ids: Vec<String>,
    /// Defines the specific data streams to subscribe to.
    pub targets: Vec<SubscribeTarget>,
}

/// `Stream` manages the lifecycle of a WebSocket connection to a Solana RPC node.
pub struct Stream {
    /// Configuration for the stream
    config: StreamConfig,
    /// Wrapper for a thread-safe callback executed on every batch
    callback: Option<BatchEventCallbackWrapper>,
    /// Tracks active requests pending server confirmation: `request_id -> TargetInfo`
    pending_requests: HashMap<u64, SubscriptionInfo>,
    /// Maps server-side subscription IDs to targets: `subscription_id -> TargetInfo`.
    subscriptions: AHashMap<u64, SubscriptionInfo>,
}

impl Stream {
    /// Creates a new `Stream` instance with the provided configuration.
    ///
    /// # Arguments
    /// * `config` - Settings for connection behavior and data processing.
    #[must_use]
    pub fn from_config(config: StreamConfig) -> Self {
        Self {
            config,
            callback: None,
            pending_requests: HashMap::new(),
            subscriptions: AHashMap::new(),
        }
    }

    /// Attaches a callback function that is triggered whenever a batch of events is ready.
    /// # Arguments
    /// * `callback` - A closure or function that processes a vector of events.
    #[must_use]
    pub fn with_callback<Callback>(mut self, callback: Callback) -> Self
    where
        Callback: FnMut(Vec<Event>) -> anyhow::Result<()> + Send + 'static,
    {
        self.callback = Some(BatchEventCallbackWrapper::new(callback));
        self
    }

    /// Starts the main subscription loop with an automatic retry mechanism.
    pub async fn subscribe(&mut self, token: CancellationToken) -> anyhow::Result<()> {
        if self.config.program_ids.is_empty() {
            bail!("Program IDs cannot be empty");
        }

        let mut delay = Duration::from_secs(1);

        while !token.is_cancelled() {
            let session_token = token.child_token();
            let start = std::time::Instant::now();

            let result = self.subscribe_session(&session_token).await;
            session_token.cancel();

            if let Err(e) = result {
                error!("Websocket Session error: {e}. Reconnecting in {delay:?}...");
            }

            tokio::select! {
                _ = token.cancelled() => break,
                _ = tokio::time::sleep(delay) => {
                    // Reset delay after a stable session, or increment backoff otherwise
                    delay = if start.elapsed() > Duration::from_secs(60) {
                        Duration::from_secs(1)
                    } else {
                        (delay * 2).min(Duration::from_secs(60))
                    };
                }
            }
        }
        Ok(())
    }

    /// Handles a single WebSocket session.
    async fn subscribe_session(&mut self, token: &CancellationToken) -> anyhow::Result<()> {
        let conf = self.config.clone();
        let (mut write, mut read) = self.connect_ws().await?;

        // Clear local state: server-side subscription IDs are invalid after reconnect
        self.pending_requests.clear();
        self.subscriptions.clear();

        // Send all subscription requests defined in config
        self.send_subscribe_requests(&mut write).await?;

        tokio::spawn(Self::heartbeat(write, conf.ping_interval, token.clone()));

        while !token.is_cancelled() {
            let mut batch = Vec::with_capacity(self.config.batch_size);
            let timeout = sleep(self.config.batch_fill_timeout);
            tokio::pin!(timeout);

            loop {
                tokio::select! {
                    _ = token.cancelled() => return Ok(()),

                    msg = read.next() => {
                        if let Some(raw) = self.handle_message(msg)? {
                            batch.push(raw);
                        }
                    }

                    _ = &mut timeout => break, // Flush batch on timeout
                }

                if batch.len() >= self.config.batch_size {
                    break;
                }
            }

            self.process_batch(batch).await?;
        }

        Ok(())
    }

    /// Establishes a new WebSocket connection to the Solana RPC node.
    async fn connect_ws(&self) -> anyhow::Result<(StreamWriter, StreamReader)> {
        let url = Url::parse(&self.config.endpoint)?;
        let (ws_stream, _) = connect_async(url.to_string())
            .await
            .context("WebSocket connection failed")?;

        Ok(ws_stream.split())
    }

    /// Sends all subscription requests to the WebSocket server based on the current configuration.
    async fn send_subscribe_requests<W>(&mut self, write: &mut W) -> anyhow::Result<()>
    where
        W: SinkExt<Message, Error = Error> + Unpin,
    {
        let requests =
            Self::build_subscribe_requests(&self.config.program_ids, &self.config.targets)?;
        for (json_val, target_info) in requests {
            // Track the request ID to match it with the server's subscription ID later
            self.pending_requests
                .insert(target_info.request_id, target_info);

            write
                .send(Message::Text(json_val.to_string().into()))
                .await?;
        }
        Ok(())
    }

    /// Constructs a list of JSON-RPC subscription requests and their associated metadata.
    fn build_subscribe_requests(
        program_ids: &[String],
        targets: &[SubscribeTarget],
    ) -> anyhow::Result<Vec<(Value, SubscriptionInfo)>> {
        let mut requests = Vec::new();
        let mut id_gen = 1..;
        let registry_entries = DEX_REGISTRY.get_all_from_strings(program_ids)?;

        for target in targets {
            match target {
                SubscribeTarget::Slot => {
                    let id = id_gen.next().unwrap();
                    requests.push(build_request(id, *target, None, &json!([])));
                }

                SubscribeTarget::Account => {
                    for (lookup, _) in registry_entries
                        .iter()
                        .filter(|(l, _)| matches!(l, RegistryLookup::Account { .. }))
                    {
                        let id = id_gen.next().unwrap();
                        let params = build_params(lookup);
                        requests.push(build_request(id, *target, Some(**lookup), &params));
                    }
                }

                SubscribeTarget::Instruction => {
                    for (lookup, _) in registry_entries
                        .iter()
                        .filter(|(l, _)| matches!(l, RegistryLookup::Instruction { .. }))
                    {
                        let id = id_gen.next().unwrap();
                        let params = build_params(lookup);
                        requests.push(build_request(id, *target, Some(**lookup), &params));
                    }
                }
            }
        }

        Ok(requests)
    }

    /// Handles an incoming WebSocket message and manages subscription state.
    fn handle_message(
        &mut self,
        msg: Option<Result<Message, Error>>,
    ) -> anyhow::Result<Option<RawMessage>> {
        match msg {
            Some(Ok(Message::Text(t))) => self.handle_text_message(&t),
            Some(Ok(Message::Binary(b))) => {
                warn!("Received unexpected binary message, length: {}", b.len());
                Ok(None)
            }
            Some(Ok(Message::Close(_))) => bail!("Websocket connection closed"),
            Some(Err(e)) => bail!("Websocket error: {e}"),
            _ => Ok(None),
        }
    }

    fn handle_text_message(&mut self, text: &Utf8Bytes) -> anyhow::Result<Option<RawMessage>> {
        let mut bytes = text.as_bytes().to_vec();
        let raw: RawMessage = simd_json::from_slice(&mut bytes)?;
        if let (Some(req_id), Some(sub_id)) = (raw.id, raw.result)
            && let Some(target) = self.pending_requests.remove(&req_id)
        {
            self.subscriptions.insert(sub_id, target);
            return Ok(None);
        }
        Ok(Some(raw))
    }

    /// Processes a batch of raw messages in parallel and triggers the callback.
    ///
    /// # Arguments
    /// * `batch` - A vector of raw messages collected during the batching window.
    async fn process_batch(&mut self, batch: Vec<RawMessage>) -> anyhow::Result<()> {
        if batch.is_empty() {
            return Ok(());
        }

        let events: Vec<Event> = batch
            .into_par_iter()
            .filter_map(|msg| Self::parse_notification(msg, &self.subscriptions))
            .collect();

        if !events.is_empty()
            && let Some(ref mut cb) = self.callback
        {
            cb.call(events).await.context("callback failed")?;
        }

        Ok(())
    }

    fn parse_notification(
        msg: RawMessage,
        subscriptions: &AHashMap<u64, SubscriptionInfo>,
    ) -> Option<Event> {
        if msg.id.is_some() {
            return None;
        }

        let method = msg.method?;
        let params = msg.params?;

        let info = subscriptions.get(&params.subscription)?;
        let sub_id = params.subscription;

        match method {
            NotificationMethod::Slot => {
                let result: SlotResult = simd_json::serde::from_owned_value(params.result).ok()?;
                Self::parse_slot(&NotificationParams {
                    subscription: sub_id,
                    result,
                })
            }
            NotificationMethod::Program => {
                let result: ProgramResult =
                    simd_json::serde::from_owned_value(params.result).ok()?;
                Self::parse_program(
                    NotificationParams {
                        subscription: sub_id,
                        result,
                    },
                    info,
                )
            }
            NotificationMethod::Logs => {
                let result: LogsResult = simd_json::serde::from_owned_value(params.result).ok()?;
                Self::parse_logs(
                    &NotificationParams {
                        subscription: sub_id,
                        result,
                    },
                    info,
                )
            }
        }
    }

    fn parse_slot(update: &NotificationParams<SlotResult>) -> Option<Event> {
        Some(Event::Slot(SlotEvent {
            slot: update.result.slot,
            parent: Some(update.result.parent),
            status: 0,
        }))
    }

    fn parse_program(
        update: NotificationParams<ProgramResult>,
        info: &SubscriptionInfo,
    ) -> Option<Event> {
        let program_id = info.program_id?;
        let result = update.result;
        let account = result.value.account;

        let payload = general_purpose::STANDARD.decode(&account.data[0]).ok()?;
        let registry_item = DEX_REGISTRY.get_account_item(&program_id, payload.len());

        let Some(item) = registry_item else {
            warn!(
                "No registered parser found for program {} with data size {}",
                program_id,
                payload.len()
            );
            return None;
        };

        let pool_state = if let DexParser::Account(parser_fn) = &item.parser {
            if let Some(state) = parser_fn(&payload) {
                state
            } else {
                error!(
                    "[{}] Failed to parse account: {}. Data size: {}",
                    item.name,
                    result.value.pubkey,
                    payload.len()
                );
                return None;
            }
        } else {
            error!(
                "Registry integrity error: Expected Account parser for {}",
                program_id
            );
            return None;
        };

        let event = AccountEvent {
            slot: result.context.slot,
            is_startup: false,
            pubkey: result.value.pubkey.parse().ok()?,
            lamports: account.lamports,
            owner: account.owner.parse().ok()?,
            executable: account.executable,
            rent_epoch: account.rent_epoch,
            write_version: 0,
            txn_signature: None,
            pool_state,
        };

        Some(Event::Account(Box::new(event)))
    }

    fn parse_logs(
        update: &NotificationParams<LogsResult>,
        info: &SubscriptionInfo,
    ) -> Option<Event> {
        let value = &update.result.value;

        if value.err.is_some() {
            return None;
        }

        let program_id = info.program_id?;

        let events: Vec<TxEvent> = value
            .logs
            .iter()
            .filter_map(|log| log.strip_prefix("Program data: "))
            .filter_map(|data_b64| {
                let payload = general_purpose::STANDARD.decode(data_b64).ok()?;

                if payload.len() < 8 {
                    return None;
                }
                let discriminator = &payload[..8];

                let item = DEX_REGISTRY.get_instruction_item(&program_id, discriminator)?;

                if let DexParser::Tx(parser_fn) = &item.parser {
                    parser_fn(&payload).or_else(|| {
                    error!(
                        "[{}] Failed to parse transaction event for program {}. Discriminator: {:?}",
                        item.name, program_id, discriminator
                    );
                        None
                    })
                } else {
                    error!("Registry integrity error: Expected Tx parser for {}", program_id);
                    None
                }
            })
            .collect();

        if events.is_empty() {
            return None;
        }

        Some(Event::Tx(events))
    }

    /// Manages the WebSocket keep-alive mechanism (L7 Pings).
    async fn heartbeat(
        mut write: SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>,
        interval_dur: Duration,
        token: CancellationToken,
    ) {
        let mut ticker = interval(interval_dur);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                _ = token.cancelled() => break,
                _ = ticker.tick() => {
                     // Send an empty Ping frame to the server
                    if let Err(e) = write.send(Message::Ping(vec![].into())).await {
                        error!("Ping heartbeat send error: {e}");
                        // Exit the loop on write error as the connection is likely dead.
                        // This allows the session task to terminate and trigger a reconnect.
                        break;
                    }
                }
            }
        }

        debug!("Heartbeat task: sending Close frame...");
        let _ = write.close().await;
    }
}

fn build_params(lookup: &RegistryLookup) -> Value {
    match lookup {
        RegistryLookup::Account { program_id, size } => {
            json!([
                program_id.to_string(),
                {
                    "encoding": "base64",
                    "filters": [
                        { "dataSize": size }
                    ]
                }
            ])
        }
        RegistryLookup::Instruction { program_id, .. } => {
            json!([
                { "mentions": [program_id.to_string()] },
                { "commitment": "processed" }
            ])
        }
    }
}

fn build_request(
    id: u64,
    target: SubscribeTarget,
    lookup: Option<RegistryLookup>,
    params: &Value,
) -> (Value, SubscriptionInfo) {
    let value = json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": target.method(),
        "params": params,
    });
    let program_id = lookup.map(|l| l.program_id());
    (value, SubscriptionInfo::new(id, program_id))
}

#[derive(Serialize, Clone, Copy, Debug)]
enum SubscribeMethod {
    #[serde(rename = "slotSubscribe")]
    Slot,
    #[serde(rename = "programSubscribe")]
    Program,
    #[serde(rename = "logsSubscribe")]
    Logs,
}

#[derive(Deserialize, Debug, PartialEq)]
pub enum NotificationMethod {
    #[serde(rename = "slotNotification")]
    Slot,
    #[serde(rename = "programNotification")]
    Program,
    #[serde(rename = "logsNotification")]
    Logs,
}

#[derive(Debug, Clone)]
pub struct SubscriptionInfo {
    pub request_id: u64,
    pub server_sub_id: Option<u64>,
    pub program_id: Option<Pubkey>,
}

impl SubscriptionInfo {
    fn new(id: u64, pk: Option<Pubkey>) -> Self {
        Self {
            request_id: id,
            server_sub_id: None,
            program_id: pk,
        }
    }
}

#[derive(Deserialize, Debug)]
pub struct RawMessage {
    pub id: Option<u64>,
    pub result: Option<u64>,
    pub method: Option<NotificationMethod>,
    pub params: Option<RawParams>,
}

#[derive(Deserialize, Debug)]
pub struct RawParams {
    pub subscription: u64,
    pub result: OwnedValue,
}

pub enum Notification {
    SlotNotification(NotificationParams<SlotResult>),
    ProgramNotification(NotificationParams<ProgramResult>),
    LogsNotification(NotificationParams<LogsResult>),
}

#[derive(Deserialize, Debug)]
pub struct NotificationParams<T> {
    pub subscription: u64,
    pub result: T,
}

#[derive(Deserialize, Debug)]
pub struct SlotResult {
    pub slot: u64,
    pub parent: u64,
    pub root: u64,
}

#[derive(Deserialize, Debug)]
pub struct ProgramResult {
    pub context: RpcContext,
    pub value: ProgramValue,
}

#[derive(Deserialize, Debug)]
pub struct ProgramValue {
    pub pubkey: String,
    pub account: AccountData,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AccountData {
    pub data: [String; 2],
    pub lamports: u64,
    pub owner: String,
    pub executable: bool,
    pub rent_epoch: u64,
    pub space: u64,
}

#[derive(Deserialize, Debug)]
pub struct LogsResult {
    pub context: RpcContext,
    pub value: LogsValue,
}

#[derive(Deserialize, Debug)]
pub struct LogsValue {
    pub signature: String,
    pub err: Option<Value>,
    pub logs: Vec<String>,
}

#[derive(Deserialize, Debug)]
pub struct RpcContext {
    pub slot: u64,
}

impl SubscribeTarget {
    fn method(self) -> SubscribeMethod {
        match self {
            Self::Slot => SubscribeMethod::Slot,
            Self::Account => SubscribeMethod::Program,
            Self::Instruction => SubscribeMethod::Logs,
        }
    }
}
