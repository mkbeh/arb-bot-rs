use solana_client::rpc_response::transaction::Signature;
use solana_sdk::{clock::Clock, pubkey::Pubkey};

use crate::libs::solana_client::{dex::*, protocols::*, registry::*};

/// Defines the types of Solana blockchain data that
/// can be subscribed to via RPC/WebSocket.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SubscribeTarget {
    /// Subscription to new slot updates.
    Slot,
    /// Subscription to program/account data changes.
    Program,
    /// Subscription to transaction logs/instructions.
    Instruction,
    /// System
    Clock,
}

/// A top-level container for all processed blockchain events.
#[derive(Debug, Clone)]
pub enum Event {
    /// Metadata about a processed block.
    BlockMeta(BlockMetaEvent),
    /// Information about a new slot.
    Slot(SlotEvent),
    /// Update of a tracked account owned by the given program or account public key.
    Program(Box<ProgramEvent>),
    /// A collection of DEX-related transaction events (e.g., Swaps).
    Tx(Vec<TxEvent>),
    /// System
    Clock(Clock),
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
    pub received_at: u64,
}

/// Describes an update to a specific Solana account,
/// including its metadata and parsed DEX state.
#[derive(Debug, Clone)]
pub struct ProgramEvent {
    pub slot: u64,
    /// Indicates if this event was generated during the
    /// initial state synchronization.
    pub is_startup: bool,
    pub pubkey: Pubkey,
    pub lamports: u64,
    pub owner: Pubkey,
    pub executable: bool,
    pub rent_epoch: u64,
    pub write_version: Option<u64>,
    /// The signature of the transaction that last modified this account.
    pub txn_signature: Option<Signature>,
    /// The high-level parsed state of the pool.
    pub pool_state: PoolState,
}

/// Represents the high-level parsed state of a Liquidity Pool
/// across different DEX protocols.
#[derive(Debug, Clone)]
pub enum PoolState {
    LbPairMeteoraDlmm(Box<meteora_dlmm::LbPair>),
    BinArrayBitmapExtensionMeteoraDlmm(Box<meteora_dlmm::BinArrayBitmapExtension>),
    BinArrayMeteoraDlmm(Box<meteora_dlmm::BinArray>),
    PoolMeteoraDammV2(Box<meteora_damm_v2::Pool>),
    PoolStateRaydiumCpmm(Box<raydium_cpmm::PoolState>),
    AmmInfoRaydiumAmm(Box<raydium_amm::AmmInfo>),
    PoolStateRaydiumClmm(Box<raydium_clmm::PoolState>),
    TickArrayBitmapExtensionRadiumClmm(Box<raydium_clmm::TickArrayBitmapExtension>),
    TickArrayStateRaydiumClmm(Box<raydium_clmm::TickArrayState>),
    WhirlpoolOrca(Box<orca::Whirlpool>),
    FixedTickArrayOrca(Box<orca::FixedTickArray>),
    DynamicTickArrayOrca(Box<orca::DynamicTickArray>),
    OracleOrca(Box<orca::Oracle>),
    BondingCurvePumpFun(Box<pump_fun::BondingCurve>),
    /// Reserves
    ReserveKamino(Box<kamino::Reserve>),
    /// Fallback for unknown or unsupported account data.
    Unknown(Vec<u8>),
}

/// Represents specific transaction events (like Swaps)
/// extracted from instruction logs.
#[derive(Debug, Clone)]
pub enum TxEvent {
    SwapMeteoraDlmm(meteora_dlmm::Swap),
    SwapMeteoraDammV2(meteora_damm_v2::Swap),
    SwapRaydiumCpmm(raydium_cpmm::Swap),
    SwapRaydiumAmm(raydium_amm::Swap),
    SwapRaydiumClmm(raydium_clmm::Swap),
    SwapOrca(orca::Swap),
    SwapPumpFun(pump_fun::Swap),
    /// Fallback for unknown or unsupported transaction instructions.
    Unknown(Vec<u8>),
}

// --- Registry Integration Implementations ---

impl<T: ProtocolEntity + 'static> ToProtocolParser<T> for PoolState {
    fn create_lookup() -> RegistryLookup {
        RegistryLookup::Program {
            program_id: T::PROGRAM_ID,
            size: T::DATA_SIZE,
            discriminator: T::DISCRIMINATOR,
        }
    }

    fn wrap_parser<F>(f: F) -> ProtocolParser
    where
        F: Fn(&[u8]) -> Option<Self> + Send + Sync + 'static,
    {
        ProtocolParser::Program(Box::new(f))
    }
}

impl<T: ProtocolEntity + 'static> ToProtocolParser<T> for TxEvent {
    fn create_lookup() -> RegistryLookup {
        RegistryLookup::Instruction {
            program_id: T::PROGRAM_ID,
            discriminator: T::DISCRIMINATOR,
        }
    }

    fn wrap_parser<F>(f: F) -> ProtocolParser
    where
        F: Fn(&[u8]) -> Option<Self> + Send + Sync + 'static,
    {
        ProtocolParser::Tx(Box::new(f))
    }
}
