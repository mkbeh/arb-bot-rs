use solana_client::rpc_response::transaction::Signature;
use solana_sdk::pubkey::Pubkey;

use crate::libs::solana_client::dex::meteora_dlmm::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SubscribeTarget {
    Slot,
    Transaction,
    Account,
}

#[derive(Debug, Clone)]
pub enum Event {
    BlockMeta(BlockMetaEvent),
    Slot(SlotEvent),
    Tx(Vec<TxEvent>),
    Account(Box<AccountEvent>),
}

#[derive(Debug, Clone)]
pub enum TxEvent {
    MeteoraDLMM(SwapMeteoraDLMM),
    Unknown(Vec<u8>), // Fallback for unknown program
}

#[derive(Debug, Clone)]
pub enum PoolState {
    MeteoraDLMM(Box<MeteoraPoolDLMM>),
    Unknown(Vec<u8>), // Fallback for unknown program
}

#[derive(Debug, Clone)]
pub struct BlockMetaEvent {
    pub slot: u64,
    pub blockhash: String,
    pub block_time: Option<u64>,
    pub block_height: Option<u64>,
    pub parent_block_hash: String,
    pub parent_slot: u64,
}

#[derive(Debug, Clone)]
pub struct SlotEvent {
    pub slot: u64,
    pub parent: Option<u64>,
    pub status: i32,
}

#[derive(Debug, Clone)]
pub struct AccountEvent {
    pub slot: u64,
    pub is_startup: bool,
    pub pubkey: Pubkey,
    pub lamports: u64,
    pub owner: Pubkey,
    pub executable: bool,
    pub rent_epoch: u64,
    pub write_version: u64,
    pub txn_signature: Option<Signature>,
    pub pool_state: PoolState,
}
