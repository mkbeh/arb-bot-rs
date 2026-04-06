use std::{collections::VecDeque, ops::BitAnd};

use bytemuck::{Pod, Zeroable};
use solana_sdk::pubkey::Pubkey;

use crate::libs::solana_client::{
    dex::raydium_clmm::{
        constants::*, error::ErrorCode, instructions::swap_internal, libraries::*, token_2022::*,
    },
    metrics::*,
    pool::*,
    registry::ProtocolEntity,
};

// Number of rewards Token
pub const REWARD_NUM: usize = 3;
pub const TICK_ARRAY_SIZE_USIZE: usize = 60;
pub const TICK_ARRAY_SIZE: i32 = 60;
pub const EXTENSION_TICKARRAY_BITMAP_SIZE: usize = 14;
pub const FEE_RATE_DENOMINATOR_VALUE: u32 = 1_000_000;

pub enum PoolStatusBitIndex {
    OpenPositionOrIncreaseLiquidity,
    DecreaseLiquidity,
    CollectFee,
    CollectReward,
    Swap,
}

/// Holds the current owner of the factory
#[repr(C, packed)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct AmmConfig {
    /// Bump to identify PDA
    pub bump: u8,
    pub index: u16,
    /// Address of the protocol owner
    pub owner: [u8; 32],
    /// The protocol fee
    pub protocol_fee_rate: u32,
    /// The trade fee, denominated in hundredths of a bip (10^-6)
    pub trade_fee_rate: u32,
    /// The tick spacing
    pub tick_spacing: u16,
    /// The fund fee, denominated in hundredths of a bip (10^-6)
    pub fund_fee_rate: u32,
    // padding space for upgrade
    pub padding_u32: u32,
    pub fund_owner: [u8; 32],
    pub padding: [u64; 3],
}

impl ProtocolEntity for AmmConfig {
    const PROGRAM_ID: Pubkey = RAYDIUM_CLMM_ID;
    const DISCRIMINATOR: &'static [u8] = &[218, 244, 33, 104, 203, 203, 43, 111];
    const DATA_SIZE: usize = 8 + 1 + 2 + 32 + 4 + 4 + 2 + 64; // 117

    fn deserialize(data: &[u8]) -> Option<Self> {
        Self::deserialize_bytemuck(data)
    }
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct PoolState {
    /// Bump to identify PDA
    pub bump: [u8; 1],
    // Which config the pool belongs
    pub amm_config: [u8; 32],
    // Pool creator
    pub owner: [u8; 32],

    /// Token pair of the pool, where token_mint_0 address < token_mint_1 address
    pub token_mint_0: [u8; 32],
    pub token_mint_1: [u8; 32],

    /// Token pair vault
    pub token_vault_0: [u8; 32],
    pub token_vault_1: [u8; 32],

    /// observation account key
    pub observation_key: [u8; 32],

    /// mint0 and mint1 decimals
    pub mint_decimals_0: u8,
    pub mint_decimals_1: u8,

    /// The minimum number of ticks between initialized ticks
    pub tick_spacing: u16,
    /// The currently in range liquidity available to the pool.
    pub liquidity: [u64; 2],
    /// The current price of the pool as a sqrt(token_1/token_0) Q64.64 value
    pub sqrt_price_x64: [u64; 2],
    /// The current tick of the pool, i.e. according to the last tick transition that was run.
    pub tick_current: i32,

    pub _padding3: u16,
    pub _padding4: u16,

    /// The fee growth as a Q64.64 number, i.e. fees of token_0 and token_1 collected per
    /// unit of liquidity for the entire life of the pool.
    pub fee_growth_global_0_x64: [u64; 2],
    pub fee_growth_global_1_x64: [u64; 2],

    /// The amounts of token_0 and token_1 that are owed to the protocol.
    pub protocol_fees_token_0: u64,
    pub protocol_fees_token_1: u64,

    /// The amounts in and out of swap token_0 and token_1
    pub swap_in_amount_token_0: [u64; 2],
    pub swap_out_amount_token_1: [u64; 2],
    pub swap_in_amount_token_1: [u64; 2],
    pub swap_out_amount_token_0: [u64; 2],

    /// Bitwise representation of the state of the pool
    /// bit0, 1: disable open position and increase liquidity, 0: normal
    /// bit1, 1: disable decrease liquidity, 0: normal
    /// bit2, 1: disable collect fee, 0: normal
    /// bit3, 1: disable collect reward, 0: normal
    /// bit4, 1: disable swap, 0: normal
    pub status: u8,
    /// Leave blank for future use
    pub _padding: [u8; 7],

    pub reward_infos: [RewardInfo; REWARD_NUM],

    /// Packed initialized tick array state
    pub tick_array_bitmap: [u64; 16],

    /// except protocol_fee and fund_fee
    pub total_fees_token_0: u64,
    /// except protocol_fee and fund_fee
    pub total_fees_claimed_token_0: u64,
    pub total_fees_token_1: u64,
    pub total_fees_claimed_token_1: u64,

    pub fund_fees_token_0: u64,
    pub fund_fees_token_1: u64,

    // The timestamp allowed for swap in the pool.
    // Note: The open_time is disabled for now.
    pub open_time: u64,
    // account recent update epoch
    pub recent_epoch: u64,

    // Unused bytes for future upgrades.
    pub _padding1: [u64; 24],
    pub _padding2: [u64; 32],
}

impl ProtocolEntity for PoolState {
    const PROGRAM_ID: Pubkey = RAYDIUM_CLMM_ID;
    const DISCRIMINATOR: &'static [u8] = &[247, 237, 227, 245, 215, 195, 222, 70];
    const DATA_SIZE: usize = 1544;

    fn deserialize(data: &[u8]) -> Option<Self> {
        Self::deserialize_bytemuck(data)
    }
}

impl DexPool for PoolState {
    fn get_mint_a(&self) -> Pubkey {
        Pubkey::from(self.token_mint_0)
    }

    fn get_mint_b(&self) -> Pubkey {
        Pubkey::from(self.token_mint_1)
    }

    fn get_amm_config_pubkey(&self) -> Option<Pubkey> {
        Some(Pubkey::from(self.amm_config))
    }

    fn quote(&self, ctx: &QuoteContext) -> anyhow::Result<QuoteResult> {
        let Some(AmmConfigType::Clmm(ref amm_config)) = ctx.amm_config else {
            anyhow::bail!("Missing AmmConfig for Raydium CLMM")
        };

        let Some(LiquidityMap::RaydiumClmm(tick_arrays)) = ctx.liquidity else {
            anyhow::bail!("Missing liquidity map for Raydium CLMM")
        };

        let bitmap_extension = match ctx.bitmap {
            Some(LiquidityBitmap::RaydiumClmm(b)) => b,
            _ => None,
        };

        let (amount, base_in) = match ctx.quote_type {
            QuoteType::ExactIn(amount) => (amount, true),
            QuoteType::ExactOut(amount) => (amount, false),
        };

        let zero_for_one = ctx.a_to_b;

        // explain: mint_in = token_mint_0, mint_out = token_mint_1
        let mint_in = ctx.unpack_mint_in()?;
        let mint_out = ctx.unpack_mint_out()?;

        let transfer_fee = if base_in {
            if zero_for_one {
                get_transfer_fee(&mint_in, ctx.clock.epoch, amount)
            } else {
                get_transfer_fee(&mint_out, ctx.clock.epoch, amount)
            }
        } else {
            0
        };

        let amount_specified = amount
            .checked_sub(transfer_fee)
            .ok_or_else(|| anyhow::anyhow!("transfer fee exceeds amount"))?;

        // get the first valid tick array index
        let (_, current_valid_tick_array_start_index) =
            self.get_first_initialized_tick_array(&bitmap_extension.copied(), zero_for_one)?;

        // filter tick arrays by direction
        let mut tick_arrays: VecDeque<&TickArrayState> = if zero_for_one {
            tick_arrays
                .iter()
                .rev()
                .filter(|(_, ta)| ta.start_tick_index <= current_valid_tick_array_start_index)
                .map(|(_, ta)| ta)
                .collect()
        } else {
            tick_arrays
                .iter()
                .filter(|(_, ta)| ta.start_tick_index >= current_valid_tick_array_start_index)
                .map(|(_, ta)| ta)
                .collect()
        };

        if tick_arrays.is_empty() {
            tracing::debug!(
                amount = amount_specified,
                zero_for_one,
                "Raydium CLMM: No tick arrays found for swap direction"
            );
            anyhow::bail!("No tick arrays available for Raydium CLMM swap");
        }

        let sqrt_price_limit_x64 = if zero_for_one {
            MIN_SQRT_PRICE_X64 + 1
        } else {
            MAX_SQRT_PRICE_X64 - 1
        };

        let (amount_0, amount_1, fee_amount, compute_units) = swap_internal(
            amm_config,
            self,
            &mut tick_arrays,
            &bitmap_extension.copied(),
            amount_specified,
            sqrt_price_limit_x64,
            zero_for_one,
            base_in,
            0,
        )?;

        let (total_amount_in_net, total_amount_out) = if zero_for_one {
            (amount_0, amount_1)
        } else {
            (amount_1, amount_0)
        };

        let transfer_fee_in = if !base_in {
            if zero_for_one {
                get_transfer_inverse_fee(&mint_in, ctx.clock.epoch, total_amount_in_net)
            } else {
                get_transfer_inverse_fee(&mint_out, ctx.clock.epoch, total_amount_in_net)
            }
        } else {
            transfer_fee
        };

        Ok(QuoteResult {
            steps: vec![],
            total_amount_in_gross: total_amount_in_net
                .checked_add(transfer_fee_in)
                .ok_or_else(|| anyhow::anyhow!("transfer fee overflow"))?,
            total_amount_in_net,
            total_amount_out,
            total_fee: fee_amount,
            compute_units,
        })
    }
}

impl ProtocolMetrics for PoolState {
    fn name(&self) -> &'static str {
        DEX_RAYDIUM_CLMM
    }
}

impl PoolState {
    #[must_use]
    pub fn sqrt_price_x64(&self) -> u128 {
        u128::from(self.sqrt_price_x64[0]) | (u128::from(self.sqrt_price_x64[1]) << 64)
    }

    #[must_use]
    pub fn liquidity(&self) -> u128 {
        u128::from(self.liquidity[0]) | (u128::from(self.liquidity[1]) << 64)
    }

    #[must_use]
    pub fn fee_growth_global_0_x64(&self) -> u128 {
        u128::from(self.fee_growth_global_0_x64[0])
            | (u128::from(self.fee_growth_global_0_x64[1]) << 64)
    }

    #[must_use]
    pub fn fee_growth_global_1_x64(&self) -> u128 {
        u128::from(self.fee_growth_global_1_x64[0])
            | (u128::from(self.fee_growth_global_1_x64[1]) << 64)
    }

    /// Get status by bit, if it is `noraml` status, return true
    #[must_use]
    pub fn get_status_by_bit(&self, bit: PoolStatusBitIndex) -> bool {
        let status = 1 << (bit as u8);
        self.status.bitand(status) == 0
    }

    #[must_use]
    pub fn is_overflow_default_tickarray_bitmap(&self, tick_indexs: Vec<i32>) -> bool {
        let (min_tick_array_start_index_boundary, max_tick_array_index_boundary) =
            self.tick_array_start_index_range();
        for tick_index in tick_indexs {
            let tick_array_start_index =
                TickArrayState::get_array_start_index(tick_index, self.tick_spacing);
            if tick_array_start_index >= max_tick_array_index_boundary
                || tick_array_start_index < min_tick_array_start_index_boundary
            {
                return true;
            }
        }
        false
    }

    pub fn get_first_initialized_tick_array(
        &self,
        tickarray_bitmap_extension: &Option<TickArrayBitmapExtension>,
        zero_for_one: bool,
    ) -> anyhow::Result<(bool, i32)> {
        let (is_initialized, start_index) =
            if self.is_overflow_default_tickarray_bitmap(vec![self.tick_current]) {
                tickarray_bitmap_extension
                    .unwrap()
                    .check_tick_array_is_initialized(
                        TickArrayState::get_array_start_index(self.tick_current, self.tick_spacing),
                        self.tick_spacing,
                    )?
            } else {
                check_current_tick_array_is_initialized(
                    U1024(self.tick_array_bitmap),
                    self.tick_current,
                    self.tick_spacing,
                )?
            };
        if is_initialized {
            return Ok((true, start_index));
        }
        let next_start_index = self.next_initialized_tick_array_start_index(
            tickarray_bitmap_extension,
            TickArrayState::get_array_start_index(self.tick_current, self.tick_spacing),
            zero_for_one,
        )?;
        anyhow::ensure!(
            next_start_index.is_some(),
            ErrorCode::InsufficientLiquidityForDirection
        );

        Ok((false, next_start_index.unwrap()))
    }

    pub fn next_initialized_tick_array_start_index(
        &self,
        tickarray_bitmap_extension: &Option<TickArrayBitmapExtension>,
        mut last_tick_array_start_index: i32,
        zero_for_one: bool,
    ) -> anyhow::Result<Option<i32>> {
        last_tick_array_start_index =
            TickArrayState::get_array_start_index(last_tick_array_start_index, self.tick_spacing);

        loop {
            let (is_found, start_index) = next_initialized_tick_array_start_index(
                U1024(self.tick_array_bitmap),
                last_tick_array_start_index,
                self.tick_spacing,
                zero_for_one,
            );
            if is_found {
                return Ok(Some(start_index));
            }
            last_tick_array_start_index = start_index;

            if tickarray_bitmap_extension.is_none() {
                anyhow::bail!(ErrorCode::MissingTickArrayBitmapExtensionAccount);
            }

            let (is_found, start_index) = tickarray_bitmap_extension
                .unwrap()
                .next_initialized_tick_array_from_one_bitmap(
                    last_tick_array_start_index,
                    self.tick_spacing,
                    zero_for_one,
                )?;
            if is_found {
                return Ok(Some(start_index));
            }
            last_tick_array_start_index = start_index;

            if !(MIN_TICK..=MAX_TICK).contains(&last_tick_array_start_index) {
                return Ok(None);
            }
        }
    }

    // the range of tick array start index that default tickarray bitmap can represent
    // if tick_spacing = 1, the result range is [-30720, 30720)
    #[must_use]
    pub fn tick_array_start_index_range(&self) -> (i32, i32) {
        // the range of ticks that default tickarrary can represent
        let mut max_tick_boundary = max_tick_in_tickarray_bitmap(self.tick_spacing);
        let mut min_tick_boundary = -max_tick_boundary;
        if max_tick_boundary > MAX_TICK {
            max_tick_boundary = TickArrayState::get_array_start_index(MAX_TICK, self.tick_spacing);
            // find the next tick array start index
            max_tick_boundary += TickArrayState::tick_count(self.tick_spacing);
        }
        if min_tick_boundary < MIN_TICK {
            min_tick_boundary = TickArrayState::get_array_start_index(MIN_TICK, self.tick_spacing);
        }
        (min_tick_boundary, max_tick_boundary)
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
/// State of reward
pub enum RewardState {
    /// Reward not initialized
    Uninitialized,
    /// Reward initialized, but reward time is not start
    Initialized,
    /// Reward in progress
    Opening,
    /// Reward end, reward time expire or
    Ended,
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct RewardInfo {
    /// Reward state
    pub reward_state: u8,
    /// Reward open time
    pub open_time: u64,
    /// Reward end time
    pub end_time: u64,
    /// Reward last update time
    pub last_update_time: u64,
    /// Q64.64 number indicates how many tokens per second are earned per unit of liquidity.
    pub emissions_per_second_x64: [u64; 2],
    /// The total amount of reward emissioned
    pub reward_total_emissioned: u64,
    /// The total amount of claimed reward
    pub reward_claimed: u64,
    /// Reward token mint.
    pub token_mint: [u8; 32],
    /// Reward vault token account.
    pub token_vault: [u8; 32],
    /// The owner that has permission to set reward param
    pub authority: [u8; 32],
    /// Q64.64 number that tracks the total tokens earned per unit of liquidity since the reward
    /// emissions were turned on.
    pub reward_growth_global_x64: [u64; 2],
}

impl RewardInfo {
    /// Returns true if this reward is initialized.
    /// Once initialized, a reward cannot transition back to uninitialized.
    #[must_use]
    pub fn initialized(&self) -> bool {
        self.token_mint.ne(&[0u8; 32])
    }
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct TickArrayBitmapExtension {
    pub pool_id: Pubkey,
    /// Packed initialized tick array state for start_tick_index is positive
    pub positive_tick_array_bitmap: [[u64; 8]; EXTENSION_TICKARRAY_BITMAP_SIZE],
    /// Packed initialized tick array state for start_tick_index is negitive
    pub negative_tick_array_bitmap: [[u64; 8]; EXTENSION_TICKARRAY_BITMAP_SIZE],
}

impl ProtocolEntity for TickArrayBitmapExtension {
    const PROGRAM_ID: Pubkey = RAYDIUM_CLMM_ID;
    const DISCRIMINATOR: &'static [u8] = &[60, 150, 36, 219, 97, 128, 139, 153];
    const DATA_SIZE: usize = 8 + 32 + 64 * EXTENSION_TICKARRAY_BITMAP_SIZE * 2; // 1832

    fn deserialize(data: &[u8]) -> Option<Self> {
        Self::deserialize_bytemuck(data)
    }
}

impl TickArrayBitmapExtension {
    fn get_bitmap_offset(tick_index: i32, tick_spacing: u16) -> anyhow::Result<usize> {
        anyhow::ensure!(
            TickArrayState::check_is_valid_start_index(tick_index, tick_spacing),
            ErrorCode::InvalidTickIndex
        );
        Self::check_extension_boundary(tick_index, tick_spacing)?;
        let ticks_in_one_bitmap = max_tick_in_tickarray_bitmap(tick_spacing);
        let mut offset = tick_index.abs() / ticks_in_one_bitmap - 1;
        if tick_index < 0 && tick_index.abs() % ticks_in_one_bitmap == 0 {
            offset -= 1;
        }
        Ok(offset as usize)
    }

    /// According to the given tick, calculate its corresponding tickarray and then find the bitmap
    /// it belongs to.
    fn get_bitmap(
        &self,
        tick_index: i32,
        tick_spacing: u16,
    ) -> anyhow::Result<(usize, TickArryBitmap)> {
        let offset = Self::get_bitmap_offset(tick_index, tick_spacing)?;
        if tick_index < 0 {
            Ok((offset, self.negative_tick_array_bitmap[offset]))
        } else {
            Ok((offset, self.positive_tick_array_bitmap[offset]))
        }
    }

    /// Search for the first initialized bit in bitmap according to the direction, if found return
    /// ture and the tick array start index, if not, return false and tick boundary index
    pub fn next_initialized_tick_array_from_one_bitmap(
        &self,
        last_tick_array_start_index: i32,
        tick_spacing: u16,
        zero_for_one: bool,
    ) -> anyhow::Result<(bool, i32)> {
        let multiplier = TickArrayState::tick_count(tick_spacing);
        let next_tick_array_start_index = if zero_for_one {
            last_tick_array_start_index - multiplier
        } else {
            last_tick_array_start_index + multiplier
        };
        let min_tick_array_start_index =
            TickArrayState::get_array_start_index(MIN_TICK, tick_spacing);
        let max_tick_array_start_index =
            TickArrayState::get_array_start_index(MAX_TICK, tick_spacing);

        if next_tick_array_start_index < min_tick_array_start_index
            || next_tick_array_start_index > max_tick_array_start_index
        {
            return Ok((false, next_tick_array_start_index));
        }

        let (_, tickarray_bitmap) = self.get_bitmap(next_tick_array_start_index, tick_spacing)?;

        Ok(Self::next_initialized_tick_array_in_bitmap(
            tickarray_bitmap,
            next_tick_array_start_index,
            tick_spacing,
            zero_for_one,
        ))
    }

    #[must_use]
    pub fn next_initialized_tick_array_in_bitmap(
        tickarray_bitmap: TickArryBitmap,
        next_tick_array_start_index: i32,
        tick_spacing: u16,
        zero_for_one: bool,
    ) -> (bool, i32) {
        let (bitmap_min_tick_boundary, bitmap_max_tick_boundary) =
            get_bitmap_tick_boundary(next_tick_array_start_index, tick_spacing);

        let tick_array_offset_in_bitmap =
            Self::tick_array_offset_in_bitmap(next_tick_array_start_index, tick_spacing);
        if zero_for_one {
            // tick from upper to lower
            // find from highter bits to lower bits
            let offset_bit_map = U512(tickarray_bitmap)
                << (TICK_ARRAY_BITMAP_SIZE - 1 - tick_array_offset_in_bitmap);

            let next_bit = if offset_bit_map.is_zero() {
                None
            } else {
                Some(u16::try_from(offset_bit_map.leading_zeros()).unwrap())
            };

            if let Some(next_bit) = next_bit {
                let next_array_start_index = next_tick_array_start_index
                    - i32::from(next_bit) * TickArrayState::tick_count(tick_spacing);
                (true, next_array_start_index)
            } else {
                // not found til to the end
                (false, bitmap_min_tick_boundary)
            }
        } else {
            // tick from lower to upper
            // find from lower bits to highter bits
            let offset_bit_map = U512(tickarray_bitmap) >> tick_array_offset_in_bitmap;

            let next_bit = if offset_bit_map.is_zero() {
                None
            } else {
                Some(u16::try_from(offset_bit_map.trailing_zeros()).unwrap())
            };
            if let Some(next_bit) = next_bit {
                let next_array_start_index = next_tick_array_start_index
                    + i32::from(next_bit) * TickArrayState::tick_count(tick_spacing);
                (true, next_array_start_index)
            } else {
                // not found til to the end
                (
                    false,
                    bitmap_max_tick_boundary - TickArrayState::tick_count(tick_spacing),
                )
            }
        }
    }

    /// Check if the tick in tick array bitmap extension
    pub fn check_extension_boundary(tick_index: i32, tick_spacing: u16) -> anyhow::Result<()> {
        let positive_tick_boundary = max_tick_in_tickarray_bitmap(tick_spacing);
        let negative_tick_boundary = -positive_tick_boundary;
        anyhow::ensure!(
            MAX_TICK > positive_tick_boundary,
            "invalid positive tick boundary"
        );
        anyhow::ensure!(
            negative_tick_boundary > MIN_TICK,
            "invalid negative tick boundary"
        );
        if tick_index >= negative_tick_boundary && tick_index < positive_tick_boundary {
            anyhow::bail!(ErrorCode::InvalidTickArrayBoundary);
        }
        Ok(())
    }

    /// Check if the tick array is initialized
    pub fn check_tick_array_is_initialized(
        &self,
        tick_array_start_index: i32,
        tick_spacing: u16,
    ) -> anyhow::Result<(bool, i32)> {
        let (_, tickarray_bitmap) = self.get_bitmap(tick_array_start_index, tick_spacing)?;

        let tick_array_offset_in_bitmap =
            Self::tick_array_offset_in_bitmap(tick_array_start_index, tick_spacing);

        if U512(tickarray_bitmap).bit(tick_array_offset_in_bitmap as usize) {
            return Ok((true, tick_array_start_index));
        }
        Ok((false, tick_array_start_index))
    }

    #[must_use]
    pub fn tick_array_offset_in_bitmap(tick_array_start_index: i32, tick_spacing: u16) -> i32 {
        let m = tick_array_start_index.abs() % max_tick_in_tickarray_bitmap(tick_spacing);
        let mut tick_array_offset_in_bitmap = m / TickArrayState::tick_count(tick_spacing);
        if tick_array_start_index < 0 && m != 0 {
            tick_array_offset_in_bitmap = TICK_ARRAY_BITMAP_SIZE - tick_array_offset_in_bitmap;
        }
        tick_array_offset_in_bitmap
    }
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct TickArrayState {
    pub pool_id: [u8; 32],
    pub start_tick_index: i32,
    pub ticks_1: [TickState; 32],
    pub ticks_2: [TickState; 28],
    pub initialized_tick_count: u8,
    pub recent_epoch: u64,
    pub _padding_1: [u8; 64],
    pub _padding_2: [u8; 32],
    pub _padding_3: [u8; 11],
}

impl ProtocolEntity for TickArrayState {
    const PROGRAM_ID: Pubkey = RAYDIUM_CLMM_ID;
    const DISCRIMINATOR: &'static [u8] = &[192, 155, 85, 205, 49, 249, 129, 42];
    const DATA_SIZE: usize = 10240;

    fn deserialize(data: &[u8]) -> Option<Self> {
        Self::deserialize_bytemuck(data)
    }
}

impl TickArrayState {
    #[must_use]
    pub fn pubkey(&self) -> Pubkey {
        Pubkey::from(self.pool_id)
    }

    #[must_use]
    pub fn get_tick(&self, idx: usize) -> Option<&TickState> {
        if idx >= TICK_ARRAY_SIZE_USIZE {
            return None;
        }

        if idx < 32 {
            Some(&self.ticks_1[idx])
        } else if idx < TICK_ARRAY_SIZE_USIZE {
            Some(&self.ticks_2[idx - 32])
        } else {
            None
        }
    }

    pub fn get_tick_mut(&mut self, index: usize) -> Option<&mut TickState> {
        if index < 32 {
            Some(&mut self.ticks_1[index])
        } else if index < 60 {
            Some(&mut self.ticks_2[index - 32])
        } else {
            None
        }
    }

    /// Base on swap directioin, return the first initialized tick in the tick array.
    pub fn first_initialized_tick(&self, zero_for_one: bool) -> anyhow::Result<&TickState> {
        if zero_for_one {
            let mut i = TICK_ARRAY_SIZE - 1;
            while i >= 0 {
                if let Some(tick) = self.get_tick(i as usize)
                    && tick.is_initialized()
                {
                    return Ok(tick);
                }
                i -= 1;
            }
        } else {
            let mut i = 0;
            while i < TICK_ARRAY_SIZE_USIZE {
                if let Some(tick) = self.get_tick(i)
                    && tick.is_initialized()
                {
                    return Ok(tick);
                }
                i += 1;
            }
        }
        anyhow::bail!(ErrorCode::InvalidTickArray)
    }

    /// Get next initialized tick in tick array, `current_tick_index` can be any tick index, in
    /// other words, `current_tick_index` not exactly a point in the tickarray,
    /// and current_tick_index % tick_spacing maybe not equal zero.
    /// If price move to left tick <= current_tick_index, or to right tick > current_tick_index
    pub fn next_initialized_tick(
        &self,
        current_tick_index: i32,
        tick_spacing: u16,
        zero_for_one: bool,
    ) -> anyhow::Result<Option<&TickState>> {
        let current_tick_array_start_index =
            Self::get_array_start_index(current_tick_index, tick_spacing);
        if current_tick_array_start_index != self.start_tick_index {
            return Ok(None);
        }
        let mut offset_in_array =
            (current_tick_index - self.start_tick_index) / i32::from(tick_spacing);

        if zero_for_one {
            while offset_in_array >= 0 {
                if let Some(tick) = self.get_tick(offset_in_array as usize)
                    && tick.is_initialized()
                {
                    return Ok(Some(tick));
                }
                offset_in_array -= 1;
            }
        } else {
            offset_in_array += 1;
            while offset_in_array < TICK_ARRAY_SIZE {
                if let Some(tick) = self.get_tick(offset_in_array as usize)
                    && tick.is_initialized()
                {
                    return Ok(Some(tick));
                }
                offset_in_array += 1;
            }
        }
        Ok(None)
    }

    /// Input an arbitrary tick_index, output the start_index of the tick_array it sits on
    #[must_use]
    pub fn get_array_start_index(tick_index: i32, tick_spacing: u16) -> i32 {
        let ticks_in_array = Self::tick_count(tick_spacing);
        let mut start = tick_index / ticks_in_array;
        if tick_index < 0 && tick_index % ticks_in_array != 0 {
            start -= 1
        }
        start * ticks_in_array
    }

    #[must_use]
    pub fn check_is_valid_start_index(tick_index: i32, tick_spacing: u16) -> bool {
        if TickState::check_is_out_of_boundary(tick_index) {
            if tick_index > MAX_TICK {
                return false;
            }
            let min_start_index = Self::get_array_start_index(MIN_TICK, tick_spacing);
            return tick_index == min_start_index;
        }
        tick_index % Self::tick_count(tick_spacing) == 0
    }

    #[must_use]
    pub fn tick_count(tick_spacing: u16) -> i32 {
        TICK_ARRAY_SIZE * i32::from(tick_spacing)
    }
}

#[repr(C, packed)]
#[derive(Default, Debug, Clone, Copy, Pod, Zeroable)]
pub struct TickState {
    pub tick: i32,
    pub liquidity_net: [i64; 2],
    pub liquidity_gross: [u64; 2],
    pub fee_growth_outside_0_x64: [u64; 2],
    pub fee_growth_outside_1_x64: [u64; 2],
    pub reward_growths_outside_x64: [[u64; 2]; REWARD_NUM],
    pub _padding: [u32; 13],
}

impl TickState {
    #[must_use]
    pub fn liquidity_net(&self) -> i128 {
        i128::from(self.liquidity_net[0]) | (i128::from(self.liquidity_net[1]) << 64)
    }

    #[must_use]
    pub fn liquidity_gross(&self) -> u128 {
        u128::from(self.liquidity_gross[0]) | (u128::from(self.liquidity_gross[1]) << 64)
    }

    /// Common checks for a valid tick input.
    /// A tick is valid if it lies within tick boundaries
    #[must_use]
    pub fn check_is_out_of_boundary(tick: i32) -> bool {
        !(MIN_TICK..=MAX_TICK).contains(&tick)
    }

    #[must_use]
    pub fn is_initialized(self) -> bool {
        self.liquidity_gross() != 0
    }
}
