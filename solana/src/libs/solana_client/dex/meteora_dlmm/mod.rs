use solana_program::{pubkey, pubkey::Pubkey};

pub mod account;
pub mod swap;

pub const METEORA_DLMM: Pubkey = pubkey!("LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo");
pub const METEORA_DLMM_POOL_SIZE: u64 = 904;
pub const METEORA_DLMM_ACCOUNT_DISCR: [u8; 8] = [33, 11, 49, 98, 181, 101, 177, 13];
pub const METEORA_DLMM_SWAP_DISCR: [u8; 8] = [81, 108, 227, 190, 205, 208, 10, 196];

pub mod prelude {

    pub use solana_program::pubkey::Pubkey;

    pub use super::{
        METEORA_DLMM, METEORA_DLMM_ACCOUNT_DISCR, METEORA_DLMM_POOL_SIZE, METEORA_DLMM_SWAP_DISCR,
        account::*, swap::*,
    };
}
