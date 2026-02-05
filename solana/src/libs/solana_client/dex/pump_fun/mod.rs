mod account;
mod swap;

pub mod constants {
    use solana_sdk::{pubkey, pubkey::Pubkey};

    pub const PUMP_FUN_ID: Pubkey = pubkey!("6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P");
}

pub use super::pump_fun::{account::BondingCurve, constants::*, swap::Swap};
