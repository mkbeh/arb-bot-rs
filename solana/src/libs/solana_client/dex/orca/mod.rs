mod account;
mod swap;

pub mod constants {
    use solana_sdk::{pubkey, pubkey::Pubkey};

    pub const ORCA_ID: Pubkey = pubkey!("whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc");
}

pub use super::orca::{account::Whirlpool, constants::*, swap::Swap};
