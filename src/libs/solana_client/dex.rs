use std::{collections::HashMap, sync::LazyLock};

use borsh::BorshDeserialize;

/// Type alias for a DEX parser function.
type DexParserFn = Box<dyn Fn(&[u8]) -> Option<TxEvent> + Send + Sync + 'static>;

/// Constant for the Raydium program ID.
pub const RAYDIUM_PROGRAM_ID: &str = "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8";
/// Constant for the Meteora program ID.
pub const METEORA_PROGRAM_ID: &str = "LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo";

/// Static map of DEX parsers.
pub static DEX_PARSERS: LazyLock<HashMap<String, DexParserFn>> = LazyLock::new(init_parsers);

/// Initializes the static DEX_PARSERS map.
fn init_parsers() -> HashMap<String, DexParserFn> {
    let mut parsers = HashMap::new();

    parsers.insert(
        RAYDIUM_PROGRAM_ID.to_string(),
        build_dex_parser::<SwapRadium>(|instr| TxEvent::Radium(Box::new(instr))),
    );

    parsers.insert(
        METEORA_PROGRAM_ID.to_string(),
        build_dex_parser::<SwapMeteora>(|instr| TxEvent::Meteora(Box::new(instr))),
    );

    parsers
}

/// Builds a DEX parser function for a specific instruction type.
fn build_dex_parser<Instr: BorshDeserialize + Clone>(
    parser: impl Fn(Instr) -> TxEvent + 'static + Send + Sync,
) -> DexParserFn {
    Box::new(move |data_bytes: &[u8]| -> Option<TxEvent> {
        match Instr::try_from_slice(data_bytes) {
            Ok(instr) => Some(parser(instr)),
            Err(_) => None, // Deserialization failure — skip this instruction.
        }
    }) as DexParserFn
}

/// Main event enum for parsed updates from the Geyser stream.
#[derive(Debug, Clone)]
#[repr(C)]
pub enum Event {
    Tx(Box<Vec<TxEvent>>),
    BlockMeta(Box<BlockMetaEvent>),
    Slot(Box<SlotEvent>),
}

/// Transaction event enum for DEX-specific parsing.
#[derive(Debug, Clone)]
pub enum TxEvent {
    Radium(Box<SwapRadium>),
    Meteora(Box<SwapMeteora>),
    Unknown(Box<Vec<u8>>), // Fallback for unknown program
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
