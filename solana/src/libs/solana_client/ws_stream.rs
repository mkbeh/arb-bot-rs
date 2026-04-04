use std::time::{Duration, Instant};

use ahash::AHashMap;
use anyhow::{Context, bail};
use async_trait::async_trait;
use base64::{Engine, engine::general_purpose};
use futures_util::{
    SinkExt, StreamExt,
    stream::{SplitSink, SplitStream},
};
use rayon::prelude::*;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use simd_json::OwnedValue;
use solana_client::client_error::reqwest::Url;
use solana_sdk::{clock::Clock, pubkey::Pubkey, sysvar::clock};
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
    SolanaStream, callback::*, metrics::*, models::*, registry::*, utils,
};

type StreamWriter = SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>;
type StreamReader = SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>;

#[derive(Clone, Debug, Default)]
pub struct WebsocketStreamConfig {
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
    /// List of protocols.
    pub protocols: ProtocolMap,
    /// Defines the specific data streams to subscribe to.
    pub targets: Vec<SubscribeTarget>,
}

/// `Stream` manages the lifecycle of a WebSocket connection to a Solana RPC node.
pub struct WebsocketStream {
    /// Configuration for the stream
    config: WebsocketStreamConfig,
    /// Wrapper for a thread-safe callback executed on every batch
    callback: Option<BatchEventCallbackWrapper>,
    /// Tracks active requests pending server confirmation: `request_id -> TargetInfo`
    pending_requests: AHashMap<u64, SubscriptionInfo>,
    /// Maps server-side subscription IDs to targets: `subscription_id -> TargetInfo`.
    subscriptions: AHashMap<u64, SubscriptionInfo>,
}

#[async_trait]
impl SolanaStream for WebsocketStream {
    fn set_callback(&mut self, callback: BatchEventCallbackWrapper) {
        self.callback = Some(callback);
    }

    async fn subscribe(&mut self, token: CancellationToken) -> anyhow::Result<()> {
        if self.config.protocols.is_empty() {
            bail!("Program IDs cannot be empty");
        }

        let mut delay = Duration::from_secs(1);

        while !token.is_cancelled() {
            let session_token = token.child_token();
            let start = Instant::now();

            let result = self.subscribe_session(&session_token).await;
            session_token.cancel();

            if let Err(e) = result {
                error!("Websocket Session error: {e}. Reconnecting in {delay:?}...");
                STREAM_METRICS.record_error(Transport::Ws, StreamErrorKind::Session);
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
}

impl WebsocketStream {
    /// Creates a new `Stream` instance with the provided configuration.
    ///
    /// # Arguments
    /// * `config` - Settings for connection behavior and data processing.
    #[must_use]
    pub fn from_config(config: WebsocketStreamConfig) -> Self {
        Self {
            config,
            callback: None,
            pending_requests: AHashMap::new(),
            subscriptions: AHashMap::new(),
        }
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
            Self::build_subscribe_requests(&self.config.protocols, &self.config.targets)?;
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
        protocol_map: &ProtocolMap,
        targets: &[SubscribeTarget],
    ) -> anyhow::Result<Vec<(Value, SubscriptionInfo)>> {
        let program_ids: Vec<String> = protocol_map.iter().map(|p| p.program_id.clone()).collect();
        let registry_entries = PROTOCOL_REGISTRY.get_all_from_strings(&program_ids)?;

        let mut requests = Vec::new();
        let mut id_gen = 1u64..;

        for target in targets {
            let new_requests = match target {
                SubscribeTarget::Clock => Self::build_sysvar_requests(&mut id_gen, clock::id()),
                SubscribeTarget::Slot => Self::build_slot_requests(&mut id_gen),
                SubscribeTarget::Program => {
                    Self::build_program_requests(&mut id_gen, &registry_entries, protocol_map)?
                }
                SubscribeTarget::Instruction => {
                    Self::build_instruction_requests(&mut id_gen, &registry_entries)
                }
            };
            requests.extend(new_requests);
        }

        Ok(requests)
    }

    fn build_slot_requests(
        id_gen: &mut impl Iterator<Item = u64>,
    ) -> Vec<(Value, SubscriptionInfo)> {
        let id = id_gen.next().unwrap();
        vec![build_request(id, SubscribeMethod::Slot, None, &json!([]))]
    }

    fn build_program_requests(
        id_gen: &mut impl Iterator<Item = u64>,
        registry_entries: &[(&RegistryLookup, &RegistryItem)],
        protocol_map: &ProtocolMap,
    ) -> anyhow::Result<Vec<(Value, SubscriptionInfo)>> {
        let mut requests = Vec::new();

        for (lookup, _) in registry_entries
            .iter()
            .filter(|(l, _)| matches!(l, RegistryLookup::Program { .. }))
        {
            let program_id_str = lookup.program_id().to_string();
            let protocol = protocol_map
                .get(&program_id_str)
                .with_context(|| format!("No protocol config found for {program_id_str}"))?;

            if protocol.account_ids.is_empty() {
                let id = id_gen.next().unwrap();
                let params = build_params(lookup);
                requests.push(build_request(
                    id,
                    SubscribeMethod::Program,
                    Some(**lookup),
                    &params,
                ));
            } else {
                for account_id in &protocol.account_ids {
                    let id = id_gen.next().unwrap();
                    let pubkey: Pubkey = account_id
                        .parse()
                        .with_context(|| format!("Invalid pubkey: {account_id}"))?;
                    let params =
                        json!([account_id, { "encoding": "base64", "commitment": "confirmed" }]);
                    let (json_val, mut info) =
                        build_request(id, SubscribeMethod::Account, Some(**lookup), &params);
                    info.account_pubkey = Some(pubkey);
                    requests.push((json_val, info));
                }
            }
        }

        Ok(requests)
    }

    fn build_instruction_requests(
        id_gen: &mut impl Iterator<Item = u64>,
        registry_entries: &[(&RegistryLookup, &RegistryItem)],
    ) -> Vec<(Value, SubscriptionInfo)> {
        registry_entries
            .iter()
            .filter(|(l, _)| matches!(l, RegistryLookup::Instruction { .. }))
            .map(|(lookup, _)| {
                let id = id_gen.next().unwrap();
                let params = build_params(lookup);
                build_request(id, SubscribeMethod::Logs, Some(**lookup), &params)
            })
            .collect()
    }

    fn build_sysvar_requests(
        id_gen: &mut impl Iterator<Item = u64>,
        pubkey: Pubkey,
    ) -> Vec<(Value, SubscriptionInfo)> {
        let id = id_gen.next().unwrap();
        let opts = json!({ "encoding": "base64", "commitment": "processed" });
        let params = json!([pubkey.to_string(), opts]);
        let (json_val, mut info) = build_request(id, SubscribeMethod::Account, None, &params);
        info.account_pubkey = Some(pubkey);
        vec![(json_val, info)]
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

        let start_time = Instant::now();
        let events: Vec<Event> = batch
            .into_par_iter()
            .filter_map(|msg| Self::parse_notification(msg, &self.subscriptions))
            .collect();

        STREAM_METRICS.record_duration(Transport::Ws, start_time);

        if !events.is_empty()
            && let Some(ref mut cb) = self.callback
        {
            let cb_start = Instant::now();
            cb.call(events).await.context("callback failed")?;
            STREAM_METRICS.record_handler_duration(cb_start);
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
        let sub_info = subscriptions.get(&params.subscription)?;

        match method {
            NotificationMethod::Slot => {
                deserialize_and_parse(params.result, |r| Self::parse_slot(&r))
            }
            NotificationMethod::Program => {
                deserialize_and_parse(params.result, |r| Self::parse_program(r, sub_info))
            }
            NotificationMethod::Account => {
                deserialize_and_parse(params.result, |r| Self::parse_account(r, sub_info))
            }
            NotificationMethod::Logs => {
                deserialize_and_parse(params.result, |r| Self::parse_logs(&r, sub_info))
            }
        }
    }

    fn parse_slot(update: &SlotResult) -> Option<Event> {
        STREAM_METRICS.record_event(Transport::Ws, EventType::Slot, "system");
        Some(Event::Slot(SlotEvent {
            slot: update.slot,
            parent: Some(update.parent),
            status: 0,
            received_at: utils::get_timestamp_ms(),
        }))
    }

    fn parse_program(update: ProgramResult, info: &SubscriptionInfo) -> Option<Event> {
        let program_id = info.program_id?;
        let pubkey = update.value.pubkey.parse().ok()?;
        let account = update.value.account;
        let payload = general_purpose::STANDARD.decode(&account.data[0]).ok()?;

        let (item, pool_state) = Self::parse_account_payload(&program_id, &pubkey, &payload)?;

        let event = ProgramEvent {
            slot: update.context.slot,
            is_startup: false,
            pubkey,
            lamports: account.lamports,
            owner: account.owner.parse().ok()?,
            executable: account.executable,
            rent_epoch: account.rent_epoch,
            write_version: None,
            txn_signature: None,
            pool_state,
        };

        STREAM_METRICS.record_event(Transport::Ws, EventType::Program, item.name);
        Some(Event::Program(Box::new(event)))
    }

    fn parse_account(update: AccountResult, info: &SubscriptionInfo) -> Option<Event> {
        let pubkey = info.account_pubkey?;
        let account = update.value;
        let payload = general_purpose::STANDARD.decode(&account.data[0]).ok()?;

        if pubkey == clock::id() {
            let clock: Clock = bincode::deserialize(&payload).ok()?;
            return Some(Event::Clock(clock));
        }

        let program_id = account.owner.parse().ok()?;
        let (item, pool_state) = Self::parse_account_payload(&program_id, &pubkey, &payload)?;

        let event = ProgramEvent {
            slot: update.context.slot,
            is_startup: false,
            pubkey,
            lamports: account.lamports,
            owner: account.owner.parse().ok()?,
            executable: account.executable,
            rent_epoch: account.rent_epoch,
            write_version: None,
            txn_signature: None,
            pool_state,
        };

        STREAM_METRICS.record_event(Transport::Ws, EventType::Program, item.name);
        Some(Event::Program(Box::new(event)))
    }

    fn parse_logs(update: &LogsResult, info: &SubscriptionInfo) -> Option<Event> {
        if update.value.err.is_some() {
            return None;
        }

        let program_id = info.program_id?;
        let events: Vec<TxEvent> = update
            .value
            .logs
            .iter()
            .filter_map(|log| log.strip_prefix("Program data: "))
            .filter_map(|data_b64| Self::parse_log_payload(data_b64, &program_id))
            .collect();

        if events.is_empty() {
            None
        } else {
            Some(Event::Tx(events))
        }
    }

    fn parse_account_payload<'a>(
        program_id: &Pubkey,
        pubkey: &Pubkey,
        payload: &[u8],
    ) -> Option<(&'a RegistryItem, PoolState)> {
        let item = PROTOCOL_REGISTRY
            .get_account_item(program_id, payload.len(), payload)
            .or_else(|| {
                STREAM_METRICS.record_error(Transport::Ws, StreamErrorKind::Parse);
                warn!(
                    "No registered parser found for program {} with data size {}",
                    program_id,
                    payload.len()
                );
                None
            })?;

        STREAM_METRICS.record_bytes(Transport::Ws, EventType::Program, item.name, payload.len());

        let ProtocolParser::Program(parser_fn) = &item.parser else {
            STREAM_METRICS.record_error(Transport::Ws, StreamErrorKind::Parse);
            error!("Registry integrity error: Expected Account parser for {program_id}");
            return None;
        };

        let pool_state = parser_fn(payload).or_else(|| {
            STREAM_METRICS.record_error(Transport::Ws, StreamErrorKind::Parse);
            error!(
                "[{}] Failed to parse account: {}. Data size: {}",
                item.name,
                pubkey,
                payload.len()
            );
            None
        })?;

        Some((item, pool_state))
    }

    fn parse_log_payload(data_b64: &str, program_id: &Pubkey) -> Option<TxEvent> {
        let payload = general_purpose::STANDARD.decode(data_b64).ok()?;
        let item = PROTOCOL_REGISTRY.get_instruction_item(program_id, &payload)?;

        STREAM_METRICS.record_bytes(Transport::Ws, EventType::Tx, item.name, payload.len());

        let ProtocolParser::Tx(parser_fn) = &item.parser else {
            STREAM_METRICS.record_error(Transport::Ws, StreamErrorKind::Parse);
            error!("Registry integrity error: Expected Tx parser for {program_id}");
            return None;
        };

        let event = parser_fn(&payload).or_else(|| {
            STREAM_METRICS.record_error(Transport::Ws, StreamErrorKind::Parse);
            error!("[{}] Failed to parse transaction event for program {program_id}. Payload: {payload:?}", item.name);
            None
        })?;

        STREAM_METRICS.record_event(Transport::Ws, EventType::Tx, item.name);
        Some(event)
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
        RegistryLookup::Program {
            program_id,
            size,
            discriminator,
            ..
        } => {
            let mut filters = Vec::new();

            if *size > 0 {
                filters.push(json!({ "dataSize": size }));
            }

            if !discriminator.is_empty() {
                filters.push(json!({
                    "memcmp": {
                        "offset": 0,
                        "bytes": bs58::encode(discriminator).into_string()
                    }
                }));
            }

            json!([
                program_id.to_string(),
                {
                    "encoding": "base64",
                    "filters": filters
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
    method: SubscribeMethod,
    lookup: Option<RegistryLookup>,
    params: &Value,
) -> (Value, SubscriptionInfo) {
    let value = json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": method,
        "params": params,
    });
    let program_id = lookup.map(|l| l.program_id());
    (value, SubscriptionInfo::new(id, program_id))
}

fn deserialize_and_parse<T, F, R>(result_value: OwnedValue, f: F) -> Option<R>
where
    T: DeserializeOwned,
    F: FnOnce(T) -> Option<R>,
{
    let result: T = simd_json::serde::from_owned_value(result_value).ok()?;
    f(result)
}

#[derive(Serialize, Clone, Copy, Debug)]
enum SubscribeMethod {
    #[serde(rename = "slotSubscribe")]
    Slot,
    #[serde(rename = "programSubscribe")]
    Program,
    #[serde(rename = "accountSubscribe")]
    Account,
    #[serde(rename = "logsSubscribe")]
    Logs,
}

#[derive(Deserialize, Debug, PartialEq)]
pub enum NotificationMethod {
    #[serde(rename = "slotNotification")]
    Slot,
    #[serde(rename = "programNotification")]
    Program,
    #[serde(rename = "accountNotification")]
    Account,
    #[serde(rename = "logsNotification")]
    Logs,
}

#[derive(Debug, Clone)]
pub struct SubscriptionInfo {
    pub request_id: u64,
    pub server_sub_id: Option<u64>,
    pub program_id: Option<Pubkey>,
    pub account_pubkey: Option<Pubkey>,
}

impl SubscriptionInfo {
    fn new(id: u64, pk: Option<Pubkey>) -> Self {
        Self {
            request_id: id,
            server_sub_id: None,
            program_id: pk,
            account_pubkey: None,
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
    pub account: AccountUpdate,
}

#[derive(Deserialize, Debug)]
pub struct AccountResult {
    pub context: RpcContext,
    pub value: AccountUpdate,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AccountUpdate {
    pub data: [String; 2],
    pub executable: bool,
    pub lamports: u64,
    pub owner: String,
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
