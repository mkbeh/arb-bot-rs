use std::{collections::HashMap, sync::LazyLock};

use borsh::BorshDeserialize;
use solana_sdk::{pubkey, pubkey::Pubkey, signature::Signature};

type ParserFn<T> = Box<dyn Fn(&[u8]) -> Option<T> + Send + Sync + 'static>;

pub const RAYDIUM_PROGRAM_ID: Pubkey = pubkey!("675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8");
pub const METEORA_PROGRAM_ID: Pubkey = pubkey!("LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo");

pub static DEX_REGISTRY: LazyLock<HashMap<Pubkey, DexParsers>> = LazyLock::new(init_dex_registry);

pub struct DexParsers {
    pub tx: ParserFn<TxEvent>,
    pub pool: ParserFn<PoolState>,
}

fn init_dex_registry() -> HashMap<Pubkey, DexParsers> {
    let mut m = HashMap::new();

    m.insert(
        RAYDIUM_PROGRAM_ID,
        DexParsers {
            tx: build_parser::<SwapRadium, TxEvent>(|i| TxEvent::Radium(Box::new(i))),
            pool: build_parser::<RaydiumPool, PoolState>(PoolState::Radium),
        },
    );

    m.insert(
        METEORA_PROGRAM_ID,
        DexParsers {
            tx: build_parser::<SwapMeteora, TxEvent>(|i| TxEvent::Meteora(Box::new(i))),
            pool: build_parser::<MeteoraPool, PoolState>(PoolState::Meteora),
        },
    );

    m
}

fn build_parser<Data, Event>(
    wrap: impl Fn(Data) -> Event + Send + Sync + 'static,
) -> ParserFn<Event>
where
    Data: BorshDeserialize + Send + Sync + 'static,
{
    Box::new(move |data| Data::try_from_slice(data).ok().map(&wrap))
}

/// Main event enum for parsed updates from the Geyser stream.
#[derive(Debug, Clone)]
#[repr(C)]
pub enum Event {
    BlockMeta(Box<BlockMetaEvent>),
    Slot(Box<SlotEvent>),
    Tx(Box<[TxEvent]>),
    Account(Box<AccountEvent>),
}

#[derive(BorshDeserialize, Debug, Clone)]
pub struct BlockMetaEvent {
    pub slot: u64,
    pub blockhash: String,
    pub block_time: Option<u64>,
    pub block_height: Option<u64>,
    pub parent_block_hash: String,
    pub parent_slot: u64,
}

#[derive(BorshDeserialize, Debug, Clone)]
pub struct SlotEvent {
    pub slot: u64,
    pub parent: Option<u64>,
    pub status: i32,
}

/// Transaction event enum for DEX-specific parsing.
#[derive(Debug, Clone)]
pub enum TxEvent {
    Radium(Box<SwapRadium>),
    Meteora(Box<SwapMeteora>),
    Unknown(Box<Vec<u8>>), // Fallback for unknown program
}

/// Struct for Radium (Raydium) swap instructions.
#[derive(BorshDeserialize, Debug, Clone)]
pub struct SwapRadium {
    pub amount_in: u64,
    pub min_amount_out: u64,
}

/// Struct for Meteora swap instructions.
#[derive(BorshDeserialize, Debug, Clone)]
pub struct SwapMeteora {
    pub amount_in: u64,
    pub min_amount_out: u64,
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

#[derive(Debug, Clone)]
pub enum PoolState {
    Radium(RaydiumPool),
    Meteora(MeteoraPool),
    Unknown(Box<Vec<u8>>), // Fallback for unknown program
}

#[derive(Debug, Clone, BorshDeserialize)]
pub struct RaydiumPool {
    // todo
}

#[derive(Debug, Clone, BorshDeserialize)]
pub struct MeteoraPool {
    // todo
}
