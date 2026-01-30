use std::{collections::HashMap, time::Duration};

use anyhow::{Context, bail};
use futures_util::SinkExt;
use rayon::{iter::ParallelIterator, prelude::*};
use solana_client::rpc_response::transaction::Signature;
use solana_sdk::pubkey::Pubkey;
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;
use tracing::{error, warn};
use yellowstone_grpc_client::{ClientTlsConfig, GeyserGrpcClient, Interceptor};
use yellowstone_grpc_proto::{
    prelude::{subscribe_update::UpdateOneof, *},
    tonic::{
        Status,
        codegen::tokio_stream::{Stream, StreamExt},
    },
};

use crate::libs::solana_client::{
    callback::BatchEventCallbackWrapper,
    models::{AccountEvent, BlockMetaEvent, Event, SlotEvent, SubscribeTarget, TxEvent},
    registry::{DEX_REGISTRY, DexParser, RegistryItem, RegistryLookup},
};

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
    /// Defines the specific data streams to subscribe to.
    pub targets: Vec<SubscribeTarget>,
    /// Options for subscription.
    pub options: Option<SubscribeOptions>,
}

/// Options for subscription.
#[derive(Clone)]
pub struct SubscribeOptions {
    /// Connect timeout
    pub connect_timeout: Duration,
    /// The interval in seconds for sending TCP
    /// keep-alive probes to the server
    pub tcp_keepalive: Duration,
    /// The interval in seconds between HTTP/2 PING frames
    pub http2_keep_alive_interval: Duration,
    /// The maximum duration in seconds to wait
    /// for a response to an HTTP/2 PING
    pub keep_alive_timeout: Duration,
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
            connect_timeout: Duration::from_secs(30),
            tcp_keepalive: Duration::from_secs(30),
            keep_alive_timeout: Duration::from_secs(60),
            http2_keep_alive_interval: Duration::from_secs(10),
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

        let mut delay = Duration::from_secs(1);

        while !token.is_cancelled() {
            let session_token = token.child_token();
            let start = std::time::Instant::now();

            let result = self.subscribe_session(&session_token).await;
            session_token.cancel();

            if let Err(e) = result {
                error!("gRPC Session error: {e}. Reconnecting in {delay:?}...");
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

    async fn subscribe_session(&self, token: &CancellationToken) -> anyhow::Result<()> {
        let config = self.config.clone();
        let options = self.config.options.clone().unwrap_or_default();

        let mut client = timeout(options.connect_timeout, Self::connect(config.clone()))
            .await
            .context("Connect timeout")?
            .context("Failed to connect to gRPC")?;

        let (mut subscribe_tx, stream) = timeout(options.connect_timeout, client.subscribe())
            .await
            .context("Subscribe timeout")?
            .context("Failed to subscribe")?;

        let request =
            Self::build_subscribe_request(&config.targets, &config.program_ids, &options)?;

        subscribe_tx
            .send(request)
            .await
            .context("Failed to send subscribe request")?;

        Self::handle_events(
            stream,
            token,
            self.callback.clone(),
            config.batch_size,
            config.batch_fill_timeout,
        )
        .await?;

        Ok(())
    }

    async fn connect(
        config: GrpcConfig,
    ) -> anyhow::Result<GeyserGrpcClient<impl Interceptor + Clone>> {
        let options = &config.options.unwrap_or_default();
        let mut builder = GeyserGrpcClient::build_from_shared(config.endpoint.clone())?
            .tcp_keepalive(Some(options.tcp_keepalive))
            .http2_keep_alive_interval(options.http2_keep_alive_interval)
            .keep_alive_timeout(options.keep_alive_timeout);

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
        targets: &[SubscribeTarget],
        program_ids: &[String],
        options: &SubscribeOptions,
    ) -> anyhow::Result<SubscribeRequest> {
        let registry_entries = DEX_REGISTRY.get_all_from_strings(program_ids)?;

        let request = SubscribeRequest {
            blocks: HashMap::new(),

            slots: if targets.contains(&SubscribeTarget::Slot) {
                Self::build_subscribe_slots()
            } else {
                HashMap::new()
            },

            accounts: if targets.contains(&SubscribeTarget::Account) {
                Self::build_subscribe_accounts(&registry_entries)
            } else {
                HashMap::new()
            },

            transactions: if targets.contains(&SubscribeTarget::Instruction) {
                Self::build_subscribe_transactions(program_ids, options)
            } else {
                HashMap::new()
            },

            commitment: options
                .commitment
                .map(|c| c as i32)
                .or(Some(CommitmentLevel::Processed as i32)),
            ..Default::default()
        };

        Ok(request)
    }

    fn build_subscribe_slots() -> HashMap<String, SubscribeRequestFilterSlots> {
        HashMap::from([(
            "slot_sub".to_owned(),
            SubscribeRequestFilterSlots {
                filter_by_commitment: Some(true),
                ..Default::default()
            },
        )])
    }

    fn build_subscribe_accounts(
        registry_entries: &[(&RegistryLookup, &RegistryItem)],
    ) -> HashMap<String, SubscribeRequestFilterAccounts> {
        registry_entries
            .iter()
            .enumerate()
            .filter_map(|(idx, (lookup, _item))| {
                let RegistryLookup::Account { program_id, size } = lookup else {
                    return None;
                };

                let filter_id = format!("acc_sub_{idx}");
                let filter = SubscribeRequestFilterAccounts {
                    owner: vec![program_id.to_string()],
                    nonempty_txn_signature: Some(true),
                    filters: vec![SubscribeRequestFilterAccountsFilter {
                        filter: Some(subscribe_request_filter_accounts_filter::Filter::Datasize(
                            *size as u64,
                        )),
                    }],
                    ..Default::default()
                };

                Some((filter_id, filter))
            })
            .collect()
    }

    fn build_subscribe_transactions(
        program_ids: &[String],
        options: &SubscribeOptions,
    ) -> HashMap<String, SubscribeRequestFilterTransactions> {
        HashMap::from([(
            "tx_sub".to_owned(),
            SubscribeRequestFilterTransactions {
                failed: if options.include_failed {
                    None
                } else {
                    Some(false)
                },
                vote: Some(options.include_vote),
                account_include: program_ids.to_vec(),
                ..Default::default()
            },
        )])
    }

    /// Processes the gRPC stream in an optimized event loop with batching and parallel parsing
    /// support.
    async fn handle_events<S>(
        mut stream: S,
        token: &CancellationToken,
        callback: Option<BatchEventCallbackWrapper>,
        batch_size: usize,
        batch_fill_timeout: Duration,
    ) -> anyhow::Result<()>
    where
        S: Stream<Item = Result<SubscribeUpdate, Status>> + Unpin + Send + 'static,
    {
        while !token.is_cancelled() {
            let mut batch = Vec::with_capacity(batch_size);

            // Wait for the first message or cancellation signal
            let msg = tokio::select! {
                _ = token.cancelled() => break,
                msg = stream.next() => match msg {
                    Some(m) => m,
                    None => bail!("Stream closed by the remote host"),
                }
            };
            batch.push(msg);

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
                .filter_map(|update| Self::parse_update(update.update_oneof.as_ref()?))
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

    /// Parses a `UpdateOneof` to an `Event`.
    fn parse_update(event: &UpdateOneof) -> Option<Event> {
        match event {
            UpdateOneof::BlockMeta(meta) => Self::parse_block_meta(meta),
            UpdateOneof::Slot(slot) => Self::parse_slot(slot),
            UpdateOneof::Transaction(tx) => Self::parse_tx(tx),
            UpdateOneof::Account(acc) => Self::parse_account(acc),
            _ => None,
        }
    }

    /// Parses a block meta update to a `TxEvent`.
    fn parse_block_meta(meta: &SubscribeUpdateBlockMeta) -> Option<Event> {
        Some(Event::BlockMeta(BlockMetaEvent {
            slot: meta.slot,
            blockhash: meta.blockhash.clone(),
            block_time: meta.block_time.as_ref().map(|ts| ts.timestamp as u64),
            block_height: meta.block_height.as_ref().map(|bh| bh.block_height),
            parent_block_hash: meta.parent_blockhash.clone(),
            parent_slot: meta.parent_slot,
        }))
    }

    /// Parses a slot update to a `TxEvent`.
    fn parse_slot(slot: &SubscribeUpdateSlot) -> Option<Event> {
        Some(Event::Slot(SlotEvent {
            slot: slot.slot,
            parent: slot.parent,
            status: slot.status,
        }))
    }

    /// Parses an account update to a `AccountEvent`.
    fn parse_account(acc: &SubscribeUpdateAccount) -> Option<Event> {
        let info = acc.account.as_ref()?;
        let owner = Pubkey::try_from(info.owner.as_slice()).ok()?;
        let pubkey = Pubkey::try_from(info.pubkey.as_slice()).ok()?;
        let payload = &info.data;

        let item = DEX_REGISTRY
            .get_account_item(&owner, payload.len())
            .or_else(|| {
                warn!(
                    "No registered parser found for program {} with data size {}",
                    owner,
                    payload.len()
                );
                None
            })?;

        let pool_state = if let DexParser::Account(parser_fn) = &item.parser {
            parser_fn(payload).or_else(|| {
                error!(
                    "[{}] Failed to parse account: {}. Data size: {}",
                    item.name,
                    pubkey,
                    payload.len()
                );
                None
            })?
        } else {
            error!(
                "Registry integrity error: Expected Account parser for {}",
                owner
            );
            return None;
        };

        let event = AccountEvent {
            slot: acc.slot,
            is_startup: acc.is_startup,
            pubkey,
            owner,
            lamports: info.lamports,
            executable: info.executable,
            rent_epoch: info.rent_epoch,
            write_version: info.write_version,
            txn_signature: info
                .txn_signature
                .as_ref()
                .and_then(|s| Signature::try_from(s.as_slice()).ok()),
            pool_state,
        };

        Some(Event::Account(Box::new(event)))
    }

    /// Parses a transaction update to a `TxEvent`.
    fn parse_tx(tx: &SubscribeUpdateTransaction) -> Option<Event> {
        let tx_info = tx.transaction.as_ref()?;
        let meta = tx_info.meta.as_ref()?;

        if tx_info.is_vote || meta.err.is_some() {
            return None;
        }

        let message = extract_message(tx)?;

        let events: Vec<TxEvent> = message
            .instructions
            .iter()
            .filter_map(|inst| {
                let program_id = extract_program_id(inst.program_id_index as usize, message, meta)?;
                let data = &inst.data;

                if data.len() < 8 {
                    return None;
                }

                let discriminator = &data[..8];

                DEX_REGISTRY.get_instruction_item(&program_id, discriminator)
                    .and_then(|item| {
                        if let DexParser::Tx(parser_fn) = &item.parser {
                            parser_fn(data).or_else(|| {
                                error!(
                                    "[{}] Failed to parse transaction for program {}. Discriminator: {:?}",
                                    item.name, program_id, discriminator
                                );
                                None
                            })
                        } else {
                            error!("Registry integrity error: Expected Tx parser for {}", program_id);
                            None
                        }
                    })
            })
            .collect();

        if events.is_empty() {
            return None;
        }
        Some(Event::Tx(events))
    }
}

fn extract_message(tx: &SubscribeUpdateTransaction) -> Option<&Message> {
    tx.transaction
        .as_ref()?
        .transaction
        .as_ref()?
        .message
        .as_ref()
}

/// Resolves a program's `Pubkey` from its index within a transaction,
/// supporting both Legacy and Versioned (v0) transactions.
///
/// In Solana v0 transactions, account keys are split into static keys (stored in the message)
/// and dynamic keys (loaded from Address Lookup Tables, stored in the meta).
fn extract_program_id(
    index: usize,
    message: &Message,
    meta: &TransactionStatusMeta,
) -> Option<Pubkey> {
    let static_len = message.account_keys.len();

    if index < static_len {
        // 1. Lookup in static account keys (Legacy part)
        // message.account_keys[index] is a Vec<u8> from gRPC proto
        Pubkey::try_from(message.account_keys[index].as_slice()).ok()
    } else {
        // 2. Lookup in dynamic account keys (ALT part from meta)
        // Offset the index by the number of static keys
        let dynamic_index = index - static_len;
        let writable_len = meta.loaded_writable_addresses.len();

        if dynamic_index < writable_len {
            // Key is in the loaded writable addresses list
            Pubkey::try_from(meta.loaded_writable_addresses[dynamic_index].as_slice()).ok()
        } else {
            // Key is in the loaded readonly addresses list
            let readonly_index = dynamic_index - writable_len;
            Pubkey::try_from(
                meta.loaded_readonly_addresses
                    .get(readonly_index)?
                    .as_slice(),
            )
            .ok()
        }
    }
}
