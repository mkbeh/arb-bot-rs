use std::{collections::HashMap, sync::Arc, time::Duration};

use anyhow::{Context, bail};
use backoff::{Error as BackoffError, ExponentialBackoff, future::retry};
use base64::{Engine, engine::general_purpose};
use futures_util::{SinkExt, TryFutureExt};
use rayon::{iter::ParallelIterator, prelude::*};
use solana_sdk::pubkey::Pubkey;
use tokio::{sync::Mutex, time::timeout};
use tokio_util::sync::CancellationToken;
use tracing::{error, warn};
use yellowstone_grpc_client::{ClientTlsConfig, GeyserGrpcClient, Interceptor};
use yellowstone_grpc_proto::{
    prelude::{
        CommitmentLevel, CompiledInstruction, Message, SubscribeRequest,
        SubscribeRequestFilterTransactions, SubscribeUpdate, SubscribeUpdateTransaction,
        subscribe_update::UpdateOneof,
    },
    tonic::{
        Status,
        codegen::tokio_stream::{Stream, StreamExt},
    },
};

use crate::libs::solana_client::{
    Event,
    dex::{BlockMetaEvent, DEX_PARSERS, SlotEvent, TxEvent},
};

type EventCallback = Box<dyn FnMut(Event) -> anyhow::Result<()> + Send + 'static>;

/// Configuration for the Solana RPC client.
#[derive(Clone, Default)]
pub struct GrpcConfig {
    /// The gRPC endpoint URL.
    pub endpoint: String,
    /// Optional API token for authenticated endpoints.
    pub x_token: Option<String>,
    /// Program IDs to subscribe to (via account_include).
    pub program_ids: Vec<String>,
    /// Options for subscription.
    pub options: Option<SubscribeOptions>,
}

/// Options for subscription.
#[derive(Clone)]
pub struct SubscribeOptions {
    /// Connect timeout.
    pub connect_timeout: u64,
    /// Include failed transactions
    pub include_failed: bool,
    /// Include vote transactions
    pub include_vote: bool,
    /// Commitment level override
    pub commitment: Option<CommitmentLevel>,
}

impl Default for SubscribeOptions {
    fn default() -> Self {
        Self {
            connect_timeout: 30,
            include_failed: false,
            include_vote: false,
            commitment: Some(CommitmentLevel::Processed),
        }
    }
}

/// Wrapper for Solana RPC gRPC client using Yellowstone Geyser protocol.
pub struct GrpcClient {
    config: GrpcConfig,
    callback: Option<EventCallbackWrapper>,
}

impl GrpcClient {
    /// Creates a new `GrpcClient` from the provided configuration.
    pub fn new(config: GrpcConfig) -> Self {
        Self {
            config,
            callback: None,
        }
    }

    /// Sets a callback for handling parsed events from the stream.
    pub fn with_callback<Callback>(mut self, callback: Callback) -> Self
    where
        Callback: FnMut(Event) -> anyhow::Result<()> + Send + 'static,
    {
        self.callback = Some(EventCallbackWrapper::new(callback));
        self
    }

    /// Subscribes to transaction updates from the specified program IDs.
    pub async fn subscribe(&mut self, token: CancellationToken) -> anyhow::Result<()> {
        if self.config.program_ids.is_empty() {
            bail!("Program IDs cannot be empty");
        }

        let operation = || {
            let token = token.clone();
            let config = self.config.clone();
            let options = self.config.options.clone().unwrap_or_default();
            let program_ids = self.config.program_ids.clone();
            let callback = self.callback.clone();

            async move {
                let mut client = timeout(
                    Duration::from_secs(options.connect_timeout),
                    Self::connect(config),
                )
                .await
                .context("Connect timeout")
                .map_err(BackoffError::transient)?
                .context("Failed to connect to gRPC")
                .map_err(BackoffError::transient)?;

                let (mut subscribe_tx, stream) = timeout(
                    Duration::from_secs(options.connect_timeout),
                    client.subscribe(),
                )
                .await
                .context("Subscribe timeout")
                .map_err(BackoffError::transient)?
                .context("Failed to subscribe")
                .map_err(BackoffError::transient)?;

                let request = Self::build_subscribe_request(program_ids, &options)
                    .map_err(BackoffError::transient)?;

                subscribe_tx
                    .send(request)
                    .await
                    .map_err(|e| BackoffError::transient(anyhow::anyhow!("Send error: {}", e)))?;

                Self::handle_events(stream, token.clone(), callback)
                    .await
                    .map_err(|e| {
                        if token.is_cancelled() {
                            BackoffError::permanent(e)
                        } else {
                            BackoffError::transient(e)
                        }
                    })?;

                Ok::<(), backoff::Error<anyhow::Error>>(())
            }
            .inspect_err(log_backoff_error)
        };

        retry(ExponentialBackoff::default(), operation).await
    }

    async fn connect(
        config: GrpcConfig,
    ) -> anyhow::Result<GeyserGrpcClient<impl Interceptor + Clone>> {
        let mut builder = GeyserGrpcClient::build_from_shared(config.endpoint.clone())?;

        // Configure TLS for secure HTTPS connections (required for official/mainnet endpoints).
        let tls_config = ClientTlsConfig::new();
        builder = builder.tls_config(tls_config)?;

        // Optionally add API token for authenticated RPC providers.
        if let Some(token) = &config.x_token {
            builder = builder.x_token(Some(token))?
        };

        builder.connect().await.map_err(Into::into)
    }

    /// Builds the initial SubscribeRequest based on program IDs and options.
    fn build_subscribe_request(
        program_ids: Vec<String>,
        options: &SubscribeOptions,
    ) -> anyhow::Result<SubscribeRequest> {
        let transactions_filter = SubscribeRequestFilterTransactions {
            failed: Some(!options.include_failed),
            vote: Some(options.include_vote),
            account_include: program_ids,
            ..Default::default()
        };

        let mut transactions = HashMap::new();
        transactions.insert("".to_owned(), transactions_filter);

        let request = SubscribeRequest {
            transactions,
            commitment: options
                .commitment
                .or(Some(CommitmentLevel::Processed))
                .map(|c| c as i32),
            accounts_data_slice: vec![],
            ..Default::default()
        };

        Ok(request)
    }

    /// Processes the gRPC stream in a blocking loop with cancellation support.
    async fn handle_events<S>(
        mut stream: S,
        token: CancellationToken,
        callback: Option<EventCallbackWrapper>,
    ) -> anyhow::Result<()>
    where
        S: Stream<Item = Result<SubscribeUpdate, Status>> + Unpin + Send + 'static,
    {
        loop {
            tokio::select! {
                _ = token.cancelled() => {
                    break;
                }

                Some(message) = stream.next() => {
                    match message {
                        Ok(update) => {
                            if let Some(oneof) = update.update_oneof.as_ref()
                                && let Some(event) = parse_update_to_event(oneof)
                                    && let Some(cb) = &callback
                                        && let Err(e) = cb.call(event).await {
                                            error!("Callback error: {}", e);
                                        }
                        }
                        Err(status) => {
                            error!("grpc stream error: {}", status);
                            bail!("grpc error: {status}");
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

/// Thread-safe wrapper for event callbacks.
#[derive(Clone)]
pub struct EventCallbackWrapper {
    inner: Arc<Mutex<EventCallback>>,
}

impl EventCallbackWrapper {
    /// Creates a new `EventCallback` from a mutable closure.
    pub fn new<F>(callback: F) -> Self
    where
        F: FnMut(Event) -> anyhow::Result<()> + Send + 'static,
    {
        Self {
            inner: Arc::new(Mutex::new(Box::new(callback))),
        }
    }

    /// Invokes the callback with the given event.
    pub async fn call(&self, event: Event) -> anyhow::Result<()> {
        let mut guard = self.inner.lock().await;
        guard(event)
    }
}

/// Parses a `UpdateOneof` to an `Event`.
fn parse_update_to_event(event: &UpdateOneof) -> Option<Event> {
    match event {
        UpdateOneof::Transaction(tx) => {
            let tx_events = parse_tx_update_to_event(tx);
            (!tx_events.is_empty()).then(|| Event::Tx(Box::new(tx_events)))
        }
        UpdateOneof::Slot(slot) => Some(Event::Slot(Box::new(SlotEvent {
            slot: slot.slot,
            parent: slot.parent,
            status: slot.status,
        }))),
        UpdateOneof::BlockMeta(meta) => Some(Event::BlockMeta(Box::new(BlockMetaEvent {
            slot: meta.slot,
            blockhash: meta.blockhash.clone(),
            block_time: meta.block_time.map(|ts| ts.timestamp as u64),
            block_height: meta.block_height.map(|bh| bh.block_height),
            parent_block_hash: meta.parent_blockhash.clone(),
            parent_slot: meta.parent_slot,
        }))),
        _ => None,
    }
}

/// Parses a transaction update to a `TxEvent`.
fn parse_tx_update_to_event(tx: &SubscribeUpdateTransaction) -> Vec<TxEvent> {
    let message = match extract_message(tx) {
        Some(msg) => msg,
        None => return vec![],
    };
    let instructions = &message.instructions;

    instructions
        .par_iter()
        .filter_map(|instruction| {
            let program_id = match extract_program_id(instruction, message) {
                Some(id) => id,
                None => return None, // Skip bad instr.
            };

            let payload = match general_purpose::STANDARD.decode(&instruction.data) {
                Ok(bytes) => bytes,
                Err(_) => return None, // Skip bad data.
            };

            DEX_PARSERS
                .get(&program_id)
                .and_then(|parser| parser(&payload)) // None if no parser or fail.
        })
        .collect()
}

fn extract_message(tx: &SubscribeUpdateTransaction) -> Option<&Message> {
    tx.transaction
        .as_ref()?
        .transaction
        .as_ref()?
        .message
        .as_ref()
}

fn extract_program_id(instruction: &CompiledInstruction, message: &Message) -> Option<String> {
    let program_id_index = instruction.program_id_index as usize;
    let key_bytes = message.account_keys.get(program_id_index)?;
    Some(Pubkey::try_from(key_bytes.as_slice()).ok()?.to_string())
}

/// Logs a `BackoffError` with type-specific level and full error chain.
fn log_backoff_error(e: &BackoffError<anyhow::Error>) {
    match e {
        BackoffError::Permanent(err) => {
            error!("PERMANENT ERROR (stopping): {:#}", err);
        }
        BackoffError::Transient {
            err,
            retry_after: _,
        } => {
            warn!("TRANSIENT ERROR (will retry): {:#}", err);
        }
    }
}
