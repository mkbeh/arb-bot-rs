use bytemuck::{Pod, Zeroable};
use solana_sdk::pubkey::Pubkey;

use crate::libs::solana_client::{
    dex::meteora_dlmm::{
        constants::*,
        quote::{quote_exact_in, quote_exact_out},
    },
    metrics::*,
    pool::*,
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

    fn quote(
        &self,
        ctx: &QuoteContext,
        data: Option<&LiquidityMap>,
    ) -> anyhow::Result<QuoteResult> {
        let Some(LiquidityMap::MeteoraDlmm(bin_arrays)) = data else {
            anyhow::bail!("Invalid data type for MeteoraDlmm");
        };

        let bitmap_extension = match ctx.bitmap {
            Some(LiquidityBitmap::MeteoraDlmm(ext)) => ext,
            _ => None,
        };

        match ctx.quote_type {
            QuoteType::ExactIn(amount) => quote_exact_in(
                self,
                amount,
                ctx.a_to_b,
                bin_arrays,
                bitmap_extension,
                ctx.clock,
                ctx.mint_in,
                ctx.mint_out,
            ),
            QuoteType::ExactOut(amount) => quote_exact_out(
                self,
                amount,
                ctx.a_to_b,
                bin_arrays,
                bitmap_extension,
                ctx.clock,
                ctx.mint_in,
                ctx.mint_out,
            ),
        }
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
    pub base_fee_power_factor: u8,
    pub _padding: [u8; 5],
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

impl Bin {
    #[must_use]
    pub fn price(&self) -> u128 {
        let bytes: &[u8] = bytemuck::bytes_of(&self.price);
        u128::from_le_bytes(bytes.try_into().unwrap())
    }

    pub fn set_price(&mut self, price: u128) {
        let bytes = price.to_le_bytes();
        self.price = *bytemuck::from_bytes(&bytes);
    }
}
