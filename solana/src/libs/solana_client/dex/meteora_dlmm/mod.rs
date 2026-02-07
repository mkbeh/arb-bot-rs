pub mod account;
pub mod swap;

pub mod constants {
    use solana_sdk::{pubkey, pubkey::Pubkey};

    pub const METEORA_DLMM_ID: Pubkey = pubkey!("LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo");

    /// Maximum number of bins array able to contains.
    pub const MAX_BINS_PER_ARRAY: usize = 70;
}

pub use super::meteora_dlmm::{
    account::{BinArray, LbPair},
    constants::*,
    swap::Swap,
};
