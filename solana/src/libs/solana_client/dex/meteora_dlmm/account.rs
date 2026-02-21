use bytemuck::{Pod, Zeroable};
use solana_sdk::pubkey::Pubkey;

use crate::libs::solana_client::{
    dex::meteora_dlmm::constants::{
        BIN_ARRAY_BITMAP_COL_COUNT, BIN_ARRAY_BITMAP_ROW_COUNT, MAX_BINS_PER_ARRAY, METEORA_DLMM_ID,
    },
    metrics::{DEX_METEORA_DLMM, DexMetrics},
    pool::{
        DexPool,
        traits::{LiquidityMap, MultiQuote, QuoteContext, QuoteError},
    },
    registry::DexEntity,
};

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct LbPair {
    pub parameters: StaticParameters,
    pub v_parameters: VariableParameters,
    pub bump_seed: [u8; 1],
    pub bin_step_seed: [u8; 2],
    pub pair_type: u8,
    pub active_id: i32,
    pub bin_step: u16,
    pub status: u8,
    pub require_base_factor_seed: u8,
    pub base_factor_seed: [u8; 2],
    pub activation_type: u8,
    pub creator_pool_on_off_control: u8,
    pub token_x_mint: [u8; 32],
    pub token_y_mint: [u8; 32],
    pub reserve_x: [u8; 32],
    pub reserve_y: [u8; 32],
    pub protocol_fee: ProtocolFee,
    pub _padding_1: [u8; 32],
    pub reward_infos: [RewardInfo; 2],
    pub oracle: [u8; 32],
    pub bin_array_bitmap: [u64; 16],
    pub last_updated_at: i64,
    pub _padding_2: [u8; 32],
    pub pre_activation_swap_address: [u8; 32],
    pub base_key: [u8; 32],
    pub activation_point: u64,
    pub pre_activation_duration: u64,
    pub _padding_3: [u8; 8],
    pub _padding_4: u64,
    pub creator: [u8; 32],
    pub token_mint_x_program_flag: u8,
    pub token_mint_y_program_flag: u8,
    pub _reserved: [u8; 22],
}

impl DexEntity for LbPair {
    const PROGRAM_ID: Pubkey = METEORA_DLMM_ID;
    const DISCRIMINATOR: &'static [u8] = &[33, 11, 49, 98, 181, 101, 177, 13];
    const DATA_SIZE: usize = 904;

    fn deserialize(data: &[u8]) -> Option<Self> {
        Self::deserialize_bytemuck(data)
    }
}

impl DexPool for LbPair {
    fn get_mint_a(&self) -> Pubkey {
        Pubkey::from(self.token_x_mint)
    }

    fn get_mint_b(&self) -> Pubkey {
        Pubkey::from(self.token_y_mint)
    }

    fn quote_out(
        &self,
        amount_in: u64,
        ctx: &QuoteContext,
        data: &LiquidityMap,
    ) -> anyhow::Result<MultiQuote, QuoteError> {
        let LiquidityMap::MeteoraDlmm(bin_arrays) = data else {
            return Err(QuoteError::InvalidPoolState);
        };

        todo!()
    }
}

impl DexMetrics for LbPair {
    fn dex_name(&self) -> &'static str {
        DEX_METEORA_DLMM
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct StaticParameters {
    pub base_factor: u16,
    pub filter_period: u16,
    pub decay_period: u16,
    pub reduction_factor: u16,
    pub variable_fee_control: u32,
    pub max_volatility_accumulator: u32,
    pub min_bin_id: i32,
    pub max_bin_id: i32,
    pub protocol_share: u16,
    pub _padding: [u8; 6],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct VariableParameters {
    pub volatility_accumulator: u32,
    pub volatility_reference: u32,
    pub index_reference: i32,
    pub _padding: [u8; 4],
    pub last_update_timestamp: i64,
    pub _padding1: [u8; 8],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct ProtocolFee {
    pub amount_x: u64,
    pub amount_y: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct RewardInfo {
    pub mint: [u8; 32],
    pub vault: [u8; 32],
    pub funder: [u8; 32],
    pub reward_per_second: u128,
    pub reward_index: u128,
    pub last_update_timestamp: i64,
    pub _padding: [u8; 8],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct BinArrayBitmapExtension {
    pub lb_pair: [u8; 32],
    /// Packed initialized bin array state for start_bin_index is positive
    pub positive_bin_array_bitmap: [[u64; BIN_ARRAY_BITMAP_COL_COUNT]; BIN_ARRAY_BITMAP_ROW_COUNT],
    /// Packed initialized bin array state for start_bin_index is negative
    pub negative_bin_array_bitmap: [[u64; BIN_ARRAY_BITMAP_COL_COUNT]; BIN_ARRAY_BITMAP_ROW_COUNT],
}

impl DexEntity for BinArrayBitmapExtension {
    const PROGRAM_ID: Pubkey = METEORA_DLMM_ID;
    const DISCRIMINATOR: &'static [u8] = &[80, 111, 124, 113, 55, 237, 18, 5];
    const DATA_SIZE: usize = 1576;

    fn deserialize(data: &[u8]) -> Option<Self> {
        Self::deserialize_bytemuck(data)
    }
}

impl BinArrayBitmapExtension {
    #[must_use]
    pub fn is_initialized(&self, index: i64) -> bool {
        // Select the appropriate bitmap and calculate the normalized bit index
        let (bitmap, bit_index) = if index >= 0 {
            (&self.positive_bin_array_bitmap, index as usize)
        } else {
            // Mapping: -1 -> 0, -2 -> 1, etc.
            (&self.negative_bin_array_bitmap, (!index) as usize)
        };

        // 1. Determine which u64 contains the bit (global word index)
        let word_idx = bit_index / 64;

        // 2. Find the row (block) index
        let row = word_idx / BIN_ARRAY_BITMAP_COL_COUNT;

        // 3. Find the column (word within the block) index
        let col = word_idx % BIN_ARRAY_BITMAP_COL_COUNT;

        // Guard against index out of bounds (exceeding the 12-row allocated memory)
        if row >= BIN_ARRAY_BITMAP_ROW_COUNT {
            return false;
        }

        // Extract the specific 64-bit word
        let word = bitmap[row][col];

        // Check the specific bit within the word
        let bit_in_word = bit_index % 64;
        (word & (1u64 << bit_in_word)) != 0
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct BinArray {
    pub index: i64,
    pub version: u8,
    pub _padding: [u8; 7],
    pub lb_pair: [u8; 32],
    pub bins_1: [Bin; 32],
    pub bins_2: [Bin; 32],
    pub bins_3: [Bin; 6],
}

impl DexEntity for BinArray {
    const PROGRAM_ID: Pubkey = METEORA_DLMM_ID;
    const DISCRIMINATOR: &'static [u8] = &[92, 142, 92, 220, 5, 148, 70, 181];
    const DATA_SIZE: usize = 10136;

    fn deserialize(data: &[u8]) -> Option<Self> {
        Self::deserialize_bytemuck(data)
    }
}

impl BinArray {
    #[must_use]
    pub fn pubkey(&self) -> Pubkey {
        Pubkey::from(self.lb_pair)
    }

    #[must_use]
    pub fn get_bin(&self, idx: usize) -> Option<&Bin> {
        if idx >= MAX_BINS_PER_ARRAY {
            return None;
        }
        match idx {
            0..=31 => Some(&self.bins_1[idx]),
            32..=63 => Some(&self.bins_2[idx - 32]),
            64..=69 => Some(&self.bins_3[idx - 64]),
            _ => unreachable!(),
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct Bin {
    pub amount_x: u64,
    pub amount_y: u64,
    pub price: [u64; 2],
    pub liquidity_supply: [u64; 2],
    pub reward_per_token_stored: [[u64; 2]; 2],
    pub fee_amount_x_per_token_stored: [u64; 2],
    pub fee_amount_y_per_token_stored: [u64; 2],
    pub amount_x_in: [u64; 2],
    pub amount_y_in: [u64; 2],
}
