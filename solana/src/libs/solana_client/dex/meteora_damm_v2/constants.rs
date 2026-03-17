use solana_sdk::{pubkey, pubkey::Pubkey};

pub const METEORA_DAMM_V2_ID: Pubkey = pubkey!("cpamdpZCGKUy5JxQXB4dcpGPiikHawvSWAd6mEn1sGG");

// Number of rewards supported by pool
pub const NUM_REWARDS: usize = 2;

pub const ONE_Q64: u128 = 1u128 << 64;

pub const MAX_RATE_LIMITER_DURATION_IN_SECONDS: u32 = 60 * 60 * 12; // 12 hours
pub const MAX_RATE_LIMITER_DURATION_IN_SLOTS: u32 = 108000; // 12 hours

/// Store constants related to fees
pub mod fee {
    use crate::libs::solana_client::dex::meteora_damm_v2::error::PoolError;

    /// Default fee denominator. DO NOT simply update it as it will break logic that depends on it
    /// as default value.
    pub const FEE_DENOMINATOR: u64 = 1_000_000_000;

    /// Max fee BPS
    pub const MAX_FEE_BPS_V0: u64 = 5000; // 50%
    pub const MAX_FEE_NUMERATOR_V0: u64 = 500_000_000; // 50%

    pub const MAX_FEE_BPS_V1: u64 = 9900; // 99%
    pub const MAX_FEE_NUMERATOR_V1: u64 = 990_000_000; // 99%

    /// Max basis point. 100% in pct
    pub const MAX_BASIS_POINT: u16 = 10_000;

    pub const MIN_FEE_NUMERATOR: u64 = 100_000;

    pub const CURRENT_POOL_VERSION: u8 = 1;

    pub fn get_max_fee_numerator(fee_version: u8) -> anyhow::Result<u64> {
        match fee_version {
            0 => Ok(MAX_FEE_NUMERATOR_V0),
            1 => Ok(MAX_FEE_NUMERATOR_V1),
            _ => Err(PoolError::InvalidPoolVersion.into()),
        }
    }

    pub fn get_max_fee_bps(fee_version: u8) -> anyhow::Result<u64> {
        match fee_version {
            0 => Ok(MAX_FEE_BPS_V0),
            1 => Ok(MAX_FEE_BPS_V1),
            _ => Err(PoolError::InvalidPoolVersion.into()),
        }
    }
}
