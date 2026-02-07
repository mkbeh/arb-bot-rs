mod account;
mod swap;

pub mod constants {
    use solana_sdk::{pubkey, pubkey::Pubkey};

    pub const ORCA_ID: Pubkey = pubkey!("whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc");

    /// Maximum number of ticks array able to contains.
    pub const TICK_ARRAY_SIZE: usize = 88;
}

pub use super::orca::{
    account::{DynamicTickArray, FixedTickArray, Whirlpool},
    constants::*,
    swap::Swap,
};
