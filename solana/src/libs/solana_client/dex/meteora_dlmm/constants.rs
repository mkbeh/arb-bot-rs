use solana_sdk::{pubkey, pubkey::Pubkey};

pub const METEORA_DLMM_ID: Pubkey = pubkey!("LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo");

pub const BASIS_POINT_MAX: i32 = 10000;

/// Maximum number of bin a bin array able to contains.
pub const MAX_BIN_PER_ARRAY: usize = 70;

/// Minimum bin ID supported. Computed based on 1 bps.
pub const MIN_BIN_ID: i32 = -443636;

/// Maximum bin ID supported. Computed based on 1 bps.
pub const MAX_BIN_ID: i32 = 443636;

/// Maximum fee rate. 10%
pub const MAX_FEE_RATE: u64 = 100_000_000;

pub const FEE_PRECISION: u64 = 1_000_000_000;

pub const BIN_ARRAY_BITMAP_ROW_COUNT: usize = 12;
pub const BIN_ARRAY_BITMAP_COL_COUNT: usize = 8;