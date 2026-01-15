use std::{collections::HashMap, sync::Arc, time::Duration};

use anyhow::{Context, bail};
use backon::{ExponentialBuilder, Retryable};
use base64::{Engine, engine::general_purpose};
use futures_util::{SinkExt, TryFutureExt};
use rayon::{iter::ParallelIterator, prelude::*};
use solana_client::rpc_response::transaction::Signature;
use solana_sdk::pubkey::Pubkey;
use tokio::{sync::Mutex, time::timeout};
use tokio_util::sync::CancellationToken;
use tracing::error;
use yellowstone_grpc_client::{ClientTlsConfig, GeyserGrpcClient, Interceptor};
use yellowstone_grpc_proto::{
    prelude::{
        CommitmentLevel, CompiledInstruction, Message, SubscribeRequest,
        SubscribeRequestFilterAccounts, SubscribeRequestFilterTransactions, SubscribeUpdate,
        SubscribeUpdateAccount, SubscribeUpdateBlockMeta, SubscribeUpdateSlot,
        SubscribeUpdateTransaction, subscribe_update::UpdateOneof,
    },
    tonic::{
        Status,
        codegen::tokio_stream::{Stream, StreamExt},
    },
};

use crate::libs::solana_client::{
    Event,
    dex::{AccountEvent, BlockMetaEvent, DEX_REGISTRY, SlotEvent, TxEvent},
};

type BatchEventCallback = Box<dyn FnMut(Vec<Event>) -> anyhow::Result<()> + Send + 'static>;

/// Configuration for the Solana RPC client.
#[derive(Clone, Default)]
pub struct GrpcConfig {
    /// The gRPC endpoint URL.
    pub endpoint: String,
    /// Optional API token for authenticated endpoints.
    pub x_token: Option<String>,
    /// Program IDs to subscribe to (via account_include).
    /// The maximum number of gRPC messages to
    /// accumulate in a single processing burst.
    pub batch_size: usize,
    /// The microsecond-grade duration to wait for additional
    /// messages after the first one arrives in the stream.
    pub batch_fill_timeout: Duration,
    /// List of Dex's programs ids.
    pub program_ids: Vec<String>,
    /// A list of specific Token Account addresses (Vaults/Reserves) to monitor.
    pub vault_addresses: Vec<String>,
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
    callback: Option<BatchEventCallbackWrapper>,
}

impl GrpcClient {
    /// Creates a new `GrpcClient` from the provided configuration.
    #[must_use]
    pub fn new(config: GrpcConfig) -> Self {
        Self {
            config,
            callback: None,
        }
    }

    /// Sets a callback for handling parsed events from the stream.
    #[must_use]
    pub fn with_callback<Callback>(mut self, callback: Callback) -> Self
    where
        Callback: FnMut(Vec<Event>) -> anyhow::Result<()> + Send + 'static,
    {
        self.callback = Some(BatchEventCallbackWrapper::new(callback));
        self
    }

    /// Subscribes to transaction updates from the specified program IDs.
    pub async fn subscribe(&mut self, token: CancellationToken) -> anyhow::Result<()> {
        if self.config.program_ids.is_empty() {
            bail!("Program IDs cannot be empty");
        }

        let ctx = SubscriptionCtx {
            config: self.config.clone(),
            options: self.config.options.clone().unwrap_or_default(),
            callback: self.callback.clone(),
        };

        let operation = || {
            let token = token.clone();
            let ctx = ctx.clone();

            async move {
                let mut client = timeout(
                    Duration::from_secs(ctx.options.connect_timeout),
                    Self::connect(ctx.config.clone()),
                )
                .await
                .context("Connect timeout")?
                .context("Failed to connect to gRPC")?;

                let (mut subscribe_tx, stream) = timeout(
                    Duration::from_secs(ctx.options.connect_timeout),
                    client.subscribe(),
                )
                .await
                .context("Subscribe timeout")?
                .context("Failed to subscribe")?;

                let request = Self::build_subscribe_request(
                    ctx.config.program_ids.clone(),
                    ctx.config.vault_addresses.clone(),
                    &ctx.options,
                );

                subscribe_tx
                    .send(request)
                    .await
                    .map_err(|e| anyhow::anyhow!("Send error: {e}"))?;

                Self::handle_events(
                    stream,
                    token.clone(),
                    ctx.callback,
                    ctx.config.batch_size,
                    ctx.config.batch_fill_timeout,
                )
                .await?;

                Ok(())
            }
            .inspect_err(|e| {
                error!(
                    error = %e,
                    "Subscription attempt failed, checking retry conditions..."
                );
            })
        };

        operation
            .retry(ExponentialBuilder::default())
            .when(|_: &anyhow::Error| !token.is_cancelled())
            .await
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
        vault_addresses: Vec<String>,
        options: &SubscribeOptions,
    ) -> SubscribeRequest {
        let accounts = if vault_addresses.is_empty() {
            HashMap::new()
        } else {
            HashMap::from([(
                "vault_sub".to_owned(),
                SubscribeRequestFilterAccounts {
                    account: vault_addresses,
                    ..Default::default()
                },
            )])
        };

        let transactions = if program_ids.is_empty() {
            HashMap::new()
        } else {
            HashMap::from([(
                "tx_sub".to_owned(),
                SubscribeRequestFilterTransactions {
                    failed: Some(!options.include_failed),
                    vote: Some(options.include_vote),
                    account_include: program_ids,
                    ..Default::default()
                },
            )])
        };

        SubscribeRequest {
            accounts,
            transactions,
            commitment: options
                .commitment
                .map(|c| c as i32)
                .or(Some(CommitmentLevel::Processed as i32)),
            ..Default::default()
        }
    }

    /// Processes the gRPC stream in an optimized event loop with batching and parallel parsing
    /// support.
    async fn handle_events<S>(
        mut stream: S,
        token: CancellationToken,
        callback: Option<BatchEventCallbackWrapper>,
        batch_size: usize,
        batch_fill_timeout: Duration,
    ) -> anyhow::Result<()>
    where
        S: Stream<Item = Result<SubscribeUpdate, Status>> + Unpin + Send + 'static,
    {
        while !token.is_cancelled() {
            let mut batch = Vec::with_capacity(batch_size);

            // Blocking wait for the initial message in the burst
            if let Some(msg) = stream.next().await {
                batch.push(msg);
            } else {
                bail!("Stream closed by the remote host");
            }

            // Fill the batch with already buffered messages
            while batch.len() < batch_size {
                // Micro-timeout ensures we don't wait for non-existent data
                // while holding up the current burst processing
                match timeout(batch_fill_timeout, stream.next()).await {
                    Ok(Some(msg)) => batch.push(msg),
                    _ => break, // Buffer empty or timeout reached
                }
            }

            // Parallel parsing of the batch
            let events: Vec<Event> = batch
                .into_par_iter()
                .filter_map(|res| res.ok())
                .filter_map(|update| parse_update(update.update_oneof.as_ref()?))
                .collect();

            if !events.is_empty()
                && let Some(cb) = &callback
                && let Err(e) = cb.call(events).await
            {
                error!(error = %e, "Batch processing error");
            }
        }

        Ok(())
    }
}

#[derive(Clone)]
struct SubscriptionCtx {
    config: GrpcConfig,
    options: SubscribeOptions,
    callback: Option<BatchEventCallbackWrapper>,
}

/// Thread-safe wrapper for event callbacks.
#[derive(Clone)]
pub struct BatchEventCallbackWrapper {
    inner: Arc<Mutex<BatchEventCallback>>,
}

impl BatchEventCallbackWrapper {
    /// Creates a new `BatchEventCallback` from a mutable closure.
    pub fn new<F>(callback: F) -> Self
    where
        F: FnMut(Vec<Event>) -> anyhow::Result<()> + Send + 'static,
    {
        Self {
            inner: Arc::new(Mutex::new(Box::new(callback))),
        }
    }

    /// Invokes the callback with the given event.
    pub async fn call(&self, events: Vec<Event>) -> anyhow::Result<()> {
        if events.is_empty() {
            return Ok(());
        }

        let mut guard = self.inner.lock().await;
        guard(events)
    }
}

/// Parses a `UpdateOneof` to an `Event`.
fn parse_update(event: &UpdateOneof) -> Option<Event> {
    match event {
        UpdateOneof::BlockMeta(meta) => parse_block_meta(meta),
        UpdateOneof::Slot(slot) => parse_slot(slot),
        UpdateOneof::Transaction(tx) => parse_tx(tx),
        UpdateOneof::Account(acc) => parse_account(acc),
        _ => None,
    }
}

/// Parses a block meta update to a `TxEvent`.
fn parse_block_meta(meta: &SubscribeUpdateBlockMeta) -> Option<Event> {
    Some(Event::BlockMeta(Box::new(BlockMetaEvent {
        slot: meta.slot,
        blockhash: meta.blockhash.clone(),
        block_time: meta.block_time.as_ref().map(|ts| ts.timestamp as u64),
        block_height: meta.block_height.as_ref().map(|bh| bh.block_height),
        parent_block_hash: meta.parent_blockhash.clone(),
        parent_slot: meta.parent_slot,
    })))
}

/// Parses a slot update to a `TxEvent`.
fn parse_slot(slot: &SubscribeUpdateSlot) -> Option<Event> {
    Some(Event::Slot(Box::new(SlotEvent {
        slot: slot.slot,
        parent: slot.parent,
        status: slot.status,
    })))
}

/// Parses a transaction update to a `TxEvent`.
fn parse_tx(tx: &SubscribeUpdateTransaction) -> Option<Event> {
    let message = extract_message(tx)?;
    let instructions = &message.instructions;

    let events: Vec<TxEvent> = instructions
        .par_iter()
        .filter_map(|instruction| {
            let program_id = extract_program_id(instruction, message)?;
            let parsers = DEX_REGISTRY.get(&program_id)?;
            let payload = general_purpose::STANDARD.decode(&instruction.data).ok()?;
            (parsers.tx)(&payload)
        })
        .collect();

    if events.is_empty() {
        None
    } else {
        Some(Event::Tx(events.into_boxed_slice()))
    }
}

/// Parses an account update to a `AccountEvent`.
fn parse_account(acc: &SubscribeUpdateAccount) -> Option<Event> {
    let account_info = acc.account.as_ref()?;
    let program_id = Pubkey::try_from(account_info.owner.as_slice()).ok()?;

    let parsers = DEX_REGISTRY.get(&program_id)?;
    let pool_state = (parsers.pool)(&account_info.data)?;

    let event = AccountEvent {
        slot: acc.slot,
        is_startup: acc.is_startup,
        pubkey: Pubkey::try_from(account_info.pubkey.as_slice()).ok()?,
        lamports: account_info.lamports,
        owner: Pubkey::try_from(account_info.owner.as_slice()).ok()?,
        executable: account_info.executable,
        rent_epoch: account_info.rent_epoch,
        write_version: account_info.write_version,
        txn_signature: account_info
            .txn_signature
            .as_ref()
            .and_then(|s| Signature::try_from(s.as_slice()).ok()),
        pool_state,
    };

    Some(Event::Account(Box::new(event)))
}

fn extract_message(tx: &SubscribeUpdateTransaction) -> Option<&Message> {
    tx.transaction
        .as_ref()?
        .transaction
        .as_ref()?
        .message
        .as_ref()
}

fn extract_program_id(instruction: &CompiledInstruction, message: &Message) -> Option<Pubkey> {
    let program_id_index = instruction.program_id_index as usize;
    let key_bytes = message.account_keys.get(program_id_index)?;
    let arr: [u8; 32] = key_bytes.as_slice().try_into().ok()?;
    Some(Pubkey::from(arr))
}
