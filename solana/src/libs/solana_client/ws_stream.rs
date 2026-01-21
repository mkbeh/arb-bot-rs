use std::{collections::HashMap, time::Duration};

use ahash::RandomState;
use anyhow::{Context, bail};
use backon::{BackoffBuilder, ExponentialBuilder};
use base64::{Engine, engine::general_purpose};
use futures_util::{SinkExt, StreamExt};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use simd_json::OwnedValue;
use solana_client::client_error::reqwest::Url;
use solana_sdk::pubkey::Pubkey;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tokio_util::sync::CancellationToken;
use tracing::error;

use crate::libs::solana_client::{
    callback::BatchEventCallbackWrapper,
    dex::{
        model::{AccountEvent, Event, SlotEvent, SubscribeTarget, TxEvent},
        registry::{DEX_REGISTRY, RegistryItem},
    },
};

pub struct StreamConfig {
    /// The gRPC endpoint URL.
    pub endpoint: String,
    /// Optional API token for authenticated endpoints.
    pub api_token: Option<(String, String)>,
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

pub struct Stream {
    config: StreamConfig,
    callback: Option<BatchEventCallbackWrapper>,
    /// request_id -> Target
    pending_requests: HashMap<u64, TargetInfo>,
    /// server_sub_id -> Target
    subscriptions: HashMap<u64, TargetInfo, RandomState>,
}

impl Stream {
    #[must_use]
    pub fn new(config: StreamConfig) -> Self {
        Self {
            config,
            callback: None,
            pending_requests: HashMap::new(),
            subscriptions: HashMap::with_hasher(RandomState::default()),
        }
    }

    #[must_use]
    pub fn with_callback<Callback>(mut self, callback: Callback) -> Self
    where
        Callback: FnMut(Vec<Event>) -> anyhow::Result<()> + Send + 'static,
    {
        self.callback = Some(BatchEventCallbackWrapper::new(callback));
        self
    }

    pub async fn subscribe(&mut self, token: CancellationToken) -> anyhow::Result<()> {
        let mut backoff = ExponentialBuilder::default().build();

        while !token.is_cancelled() {
            match self.subscribe_session(&token).await {
                Ok(_) => break,
                Err(e) => {
                    error!("Stream error: {}. Retrying...", e);
                    let delay = backoff.next().unwrap_or(Duration::from_secs(10));

                    tokio::select! {
                        _ = token.cancelled() => break,
                        _ = tokio::time::sleep(delay) => {},
                    }
                }
            }
        }
        Ok(())
    }

    async fn subscribe_session(&mut self, token: &CancellationToken) -> anyhow::Result<()> {
        let url = build_url(&self.config.endpoint, self.config.api_token.clone())?;
        let (ws_stream, _) = connect_async(url.to_string())
            .await
            .context("Failed to connect")?;

        let (mut write, mut read) = ws_stream.split();
        let mut ping_interval = tokio::time::interval(self.config.ping_interval);

        self.subscriptions.clear();
        self.pending_requests.clear();

        self.send_subscribe_requests(&mut write).await?;

        while !token.is_cancelled() {
            let mut batch = Vec::with_capacity(self.config.batch_size);
            let timeout = tokio::time::sleep(self.config.batch_fill_timeout);
            tokio::pin!(timeout);

            loop {
                tokio::select! {
                    _ = token.cancelled() => return Ok(()),

                    _ = ping_interval.tick() => {
                        write.send(Message::Ping(vec![].into())).await?;
                    }

                    msg = read.next() => {
                        let msg = match msg {
                            Some(Ok(m)) => m,
                            Some(Err(e)) => bail!("Stream error: {e}"),
                            None => bail!("Server closed connection"),
                        };

                        if let Some(raw) = self.handle_message(msg).await? {
                            batch.push(raw);
                        }
                    }

                    _ = &mut timeout => break,
                }

                if batch.len() >= self.config.batch_size {
                    break;
                }
            }

            self.process_batch(batch).await?;
        }
        Ok(())
    }

    async fn send_subscribe_requests<W>(&mut self, write: &mut W) -> anyhow::Result<()>
    where
        W: SinkExt<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
    {
        let requests =
            Self::build_subscribe_requests(&self.config.program_ids, &self.config.targets);
        for (json_val, target_info) in requests {
            self.pending_requests
                .insert(target_info.request_id, target_info);
            write
                .send(Message::Text(json_val.to_string().into()))
                .await?;
        }
        Ok(())
    }

    async fn handle_message(&mut self, msg: Message) -> anyhow::Result<Option<RawMessage>> {
        match msg {
            Message::Text(text) => {
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
            Message::Close(_) => bail!("Websocket connection closed"),
            _ => Ok(None),
        }
    }

    async fn process_batch(&mut self, batch: Vec<RawMessage>) -> anyhow::Result<()> {
        if batch.is_empty() {
            return Ok(());
        }

        let events: Vec<Event> = batch
            .into_par_iter()
            .filter_map(|msg| parse_notification(msg, &self.subscriptions))
            .collect();

        if let Some(ref mut cb) = self.callback {
            cb.call(events).await?;
        }
        Ok(())
    }

    fn build_subscribe_requests(
        program_ids: &[String],
        targets: &[SubscribeTarget],
    ) -> Vec<(Value, TargetInfo)> {
        let valid_pubkeys: Vec<(&Pubkey, &RegistryItem)> = program_ids
            .iter()
            .filter_map(|id| {
                let pk = id.parse::<Pubkey>().ok()?;
                DEX_REGISTRY.get_key_value(&pk)
            })
            .collect();

        let mut requests = Vec::new();
        let mut next_id = 1..;

        for target in targets {
            match target {
                SubscribeTarget::Slot => {
                    let id = next_id.next().unwrap();
                    requests.push((
                        build_subscribe_value(id, SubscribeMethod::Slot, &json!([])),
                        TargetInfo::new(id, *target, None),
                    ));
                }

                SubscribeTarget::Account | SubscribeTarget::Transaction => {
                    for (pk, config) in &valid_pubkeys {
                        let id = next_id.next().unwrap();

                        let (method, params) = match target {
                            SubscribeTarget::Account => (
                                SubscribeMethod::Program,
                                json!([pk.to_string(), {
                                    "encoding": "base64",
                                    "filters": [{"dataSize": config.pool_size}]
                                }]),
                            ),
                            _ => (
                                SubscribeMethod::Logs,
                                json!([
                                    {"mentions": [pk.to_string()]},
                                    {"commitment": "processed"}
                                ]),
                            ),
                        };

                        requests.push((
                            build_subscribe_value(id, method, &params),
                            TargetInfo::new(id, *target, Some(**pk)),
                        ));
                    }
                }
            }
        }

        requests
    }
}

fn build_url(endpoint: &str, api_token: Option<(String, String)>) -> anyhow::Result<Url> {
    let mut url = Url::parse(endpoint)?;

    if let Some(api_token) = api_token {
        let (key, secret) = &api_token;
        url.query_pairs_mut().append_pair(key, secret);
    }

    Ok(url)
}

fn build_subscribe_value(id: u64, method: SubscribeMethod, params: &Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": method,
        "params": params,
    })
}

fn parse_notification(
    msg: RawMessage,
    subscriptions: &HashMap<u64, TargetInfo, RandomState>,
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
            parse_slot(&NotificationParams {
                subscription: sub_id,
                result,
            })
        }
        NotificationMethod::Program => {
            let result: ProgramResult = simd_json::serde::from_owned_value(params.result).ok()?;
            parse_program(
                NotificationParams {
                    subscription: sub_id,
                    result,
                },
                info,
            )
        }
        NotificationMethod::Logs => {
            let result: LogsResult = simd_json::serde::from_owned_value(params.result).ok()?;
            parse_logs(
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

fn parse_program(update: NotificationParams<ProgramResult>, info: &TargetInfo) -> Option<Event> {
    let dex_conf = info.program_id.and_then(|pk| DEX_REGISTRY.get(&pk))?;
    let result = update.result;
    let account = result.value.account;

    let payload = general_purpose::STANDARD.decode(&account.data[0]).ok()?;
    let Some(pool_state) = (dex_conf.parser.pool)(&payload) else {
        error!(
            "[{}] Failed to parse pool state for account: {}\n\
                Payload: {:?}",
            dex_conf.name, result.value.pubkey, account.data,
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

fn parse_logs(update: &NotificationParams<LogsResult>, info: &TargetInfo) -> Option<Event> {
    let value = &update.result.value;

    if value.err.is_some() {
        return None;
    }

    let dex_conf = info.program_id.and_then(|pk| DEX_REGISTRY.get(&pk))?;
    let events: Vec<TxEvent> = value
        .logs
        .iter()
        .filter_map(|log| log.strip_prefix("Program data: "))
        .filter_map(|data_b64| {
            let payload = general_purpose::STANDARD.decode(data_b64).ok()?;
            (dex_conf.parser.tx)(&payload)
        })
        .collect();

    if events.is_empty() {
        return None;
    }

    Some(Event::Tx(events))
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
pub struct TargetInfo {
    pub target: SubscribeTarget,
    pub request_id: u64,
    pub server_sub_id: Option<u64>,
    pub program_id: Option<Pubkey>,
}

impl TargetInfo {
    fn new(id: u64, target: SubscribeTarget, pk: Option<Pubkey>) -> Self {
        Self {
            target,
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
