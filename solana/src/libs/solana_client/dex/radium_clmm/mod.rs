mod account;
mod swap;

pub mod constants {
    use solana_sdk::{pubkey, pubkey::Pubkey};

    pub const RAYDIUM_CLMM_ID: Pubkey = pubkey!("CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK");

    // Number of rewards Token
    pub const REWARD_NUM: usize = 3;

    // Maximum number of ticks array able to contains.
    pub const TICK_ARRAY_SIZE_USIZE: usize = 60;
}

pub use super::radium_clmm::{
    account::{PoolState, TickArrayState},
    constants::*,
    swap::Swap,
};
