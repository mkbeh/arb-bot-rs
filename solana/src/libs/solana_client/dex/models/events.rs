use solana_client::rpc_response::transaction::Signature;
use solana_sdk::pubkey::Pubkey;

use crate::libs::solana_client::dex::{
    meteora_dlmm,
    parser::DexEntity,
    radium_cpmm,
    registry::{DexParser, RegistryLookup, ToDexParser},
};

/// Defines the types of Solana blockchain data that
/// can be subscribed to via RPC/WebSocket.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SubscribeTarget {
    /// Subscription to new slot updates.
    Slot,
    /// Subscription to transaction logs/instructions.
    Instruction,
    /// Subscription to account data changes.
    Account,
}

/// A top-level container for all processed blockchain events.
#[derive(Debug, Clone)]
pub enum Event {
    /// Metadata about a processed block.
    BlockMeta(BlockMetaEvent),
    /// Information about a new slot.
    Slot(SlotEvent),
    /// Update of a tracked DEX account state.
    Account(Box<AccountEvent>),
    /// A collection of DEX-related transaction events (e.g., Swaps).
    Tx(Vec<TxEvent>),
}

/// Detailed metadata for a confirmed Solana block.
#[derive(Debug, Clone)]
pub struct BlockMetaEvent {
    pub slot: u64,
    pub blockhash: String,
    pub block_time: Option<u64>,
    pub block_height: Option<u64>,
    pub parent_block_hash: String,
    pub parent_slot: u64,
}

/// Represents a slot update event.
#[derive(Debug, Clone)]
pub struct SlotEvent {
    pub slot: u64,
    pub parent: Option<u64>,
    pub status: i32,
}

/// Describes an update to a specific Solana account,
/// including its metadata and parsed DEX state.
#[derive(Debug, Clone)]
pub struct AccountEvent {
    pub slot: u64,
    /// Indicates if this event was generated during the
    /// initial state synchronization.
    pub is_startup: bool,
    pub pubkey: Pubkey,
    pub lamports: u64,
    pub owner: Pubkey,
    pub executable: bool,
    pub rent_epoch: u64,
    pub write_version: u64,
    /// The signature of the transaction that last modified this account.
    pub txn_signature: Option<Signature>,
    /// The high-level parsed state of the pool.
    pub pool_state: PoolState,
}

/// Represents the high-level parsed state of a Liquidity Pool
/// across different DEX protocols.
#[derive(Debug, Clone)]
pub enum PoolState {
    LbPairMeteoraDlmm(meteora_dlmm::LbPair),
    PoolStateRadiumCpmm(radium_cpmm::PoolState),
    /// Fallback for unknown or unsupported account data.
    Unknown(Vec<u8>),
}

/// Represents specific transaction events (like Swaps)
/// extracted from instruction logs.
#[derive(Debug, Clone)]
pub enum TxEvent {
    SwapMeteoraDlmm(meteora_dlmm::Swap),
    SwapRadiumCpmm(radium_cpmm::Swap),
    /// Fallback for unknown or unsupported transaction instructions.
    Unknown(Vec<u8>),
}

// --- Registry Integration Implementations ---

impl<T: DexEntity + 'static> ToDexParser<T> for PoolState {
    fn create_lookup() -> RegistryLookup {
        RegistryLookup::Account {
            program_id: T::PROGRAM_ID,
            size: T::POOL_SIZE,
        }
    }

    fn wrap_parser<F>(f: F) -> DexParser
    where
        F: Fn(&[u8]) -> Option<Self> + Send + Sync + 'static,
    {
        DexParser::Account(Box::new(f))
    }
}

impl<T: DexEntity + 'static> ToDexParser<T> for TxEvent {
    fn create_lookup() -> RegistryLookup {
        RegistryLookup::Instruction {
            program_id: T::PROGRAM_ID,
            discriminator: T::DISCRIMINATOR,
        }
    }

    fn wrap_parser<F>(f: F) -> DexParser
    where
        F: Fn(&[u8]) -> Option<Self> + Send + Sync + 'static,
    {
        DexParser::Tx(Box::new(f))
    }
}
