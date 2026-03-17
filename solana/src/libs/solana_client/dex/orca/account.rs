use bytemuck::{Pod, Zeroable};
use num_traits::ToPrimitive;
use orca_whirlpools_core::{
    MAX_TICK_INDEX, MIN_TICK_INDEX, NUM_REWARDS, OracleFacade, TICK_ARRAY_SIZE, TickArrayFacade,
    TickArrays, TickFacade, WhirlpoolFacade, WhirlpoolRewardInfoFacade, swap_quote_by_input_token,
    swap_quote_by_output_token, try_apply_transfer_fee,
};
use solana_sdk::pubkey::Pubkey;
use tracing::error;

use crate::libs::solana_client::{
    dex::orca::{constants::*, math::floor_division, token::get_epoch_transfer_fee},
    metrics::*,
    pool::*,
    registry::DexEntity,
};

const ORCA_COMPUTE_UNITS: u32 = 75_000;

#[repr(C, packed)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct Whirlpool {
    pub whirlpools_config: [u8; 32], // 32
    pub whirlpool_bump: [u8; 1],     // 1

    pub tick_spacing: u16,            // 2
    pub fee_tier_index_seed: [u8; 2], // 2

    // Stored as hundredths of a basis point
    // u16::MAX corresponds to ~6.5%
    pub fee_rate: u16, // 2

    // Portion of fee rate taken stored as basis points
    pub protocol_fee_rate: u16, // 2

    // Maximum amount that can be held by Solana account
    pub liquidity: [u64; 2], // 16

    // MAX/MIN at Q32.64, but using Q64.64 for rounder bytes
    // Q64.64
    pub sqrt_price: [u64; 2],    // 16
    pub tick_current_index: i32, // 4

    pub protocol_fee_owed_a: u64, // 8
    pub protocol_fee_owed_b: u64, // 8

    pub token_mint_a: [u8; 32],  // 32
    pub token_vault_a: [u8; 32], // 32

    // Q64.64
    pub fee_growth_global_a: [u64; 2], // 16

    pub token_mint_b: [u8; 32],  // 32
    pub token_vault_b: [u8; 32], // 32

    // Q64.64
    pub fee_growth_global_b: [u64; 2], // 16

    pub reward_last_updated_timestamp: u64, // 8

    pub reward_infos: [WhirlpoolRewardInfo; NUM_REWARDS], // 384
}

impl DexEntity for Whirlpool {
    const PROGRAM_ID: Pubkey = ORCA_ID;
    const DISCRIMINATOR: &'static [u8] = &[63, 149, 209, 12, 225, 128, 99, 9];
    const DATA_SIZE: usize = 8 + 261 + 384; // 653

    fn deserialize(data: &[u8]) -> Option<Self> {
        Self::deserialize_bytemuck(data)
    }
}

impl DexPool for Whirlpool {
    fn get_mint_a(&self) -> Pubkey {
        Pubkey::from(self.token_mint_a)
    }

    fn get_mint_b(&self) -> Pubkey {
        Pubkey::from(self.token_mint_b)
    }

    fn quote(&self, ctx: &QuoteContext) -> anyhow::Result<QuoteResult> {
        let liquidity = ctx
            .liquidity
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Orca Whirlpool: missing tick arrays"))?;

        let whirlpool_facade = WhirlpoolFacade::from(self);
        let oracle_facade = ctx.oracle.map(OracleFacade::from);
        let timestamp = ctx.clock.unix_timestamp.to_u64().unwrap();

        let start_indexes =
            get_start_tick_indexes(self.tick_current_index, self.tick_spacing, ctx.a_to_b);

        let facades: Vec<TickArrayFacade> = start_indexes
            .iter()
            .filter_map(|&idx| match liquidity {
                LiquidityMap::OrcaFixed(map) => map.get(&idx).map(TickArrayFacade::from),
                LiquidityMap::OrcaDynamic(map) => map.get(&idx).map(TickArrayFacade::from),
                _ => None,
            })
            .collect();

        let tick_arrays = match facades.as_slice() {
            [a] => TickArrays::One(*a),
            [a, b] => TickArrays::Two(*a, *b),
            [a, b, c] | [a, b, c, ..] => TickArrays::Three(*a, *b, *c),
            _ => anyhow::bail!("Orca Whirlpool: no tick arrays found"),
        };

        let transfer_fee_a = get_epoch_transfer_fee(&ctx.unpack_mint_in()?, ctx.clock.epoch);
        let transfer_fee_b = get_epoch_transfer_fee(&ctx.unpack_mint_out()?, ctx.clock.epoch);
        let transfer_fee_in = if ctx.a_to_b {
            transfer_fee_a
        } else {
            transfer_fee_b
        };

        match ctx.quote_type {
            QuoteType::ExactIn(amount) => {
                let result = swap_quote_by_input_token(
                    amount,
                    ctx.a_to_b,
                    0,
                    whirlpool_facade,
                    oracle_facade,
                    tick_arrays,
                    timestamp,
                    transfer_fee_a,
                    transfer_fee_b,
                )
                .map_err(|e| anyhow::anyhow!("Orca ExactIn quote error: {e}"))?;

                let total_amount_in_net =
                    try_apply_transfer_fee(result.token_in, transfer_fee_in.unwrap_or_default())
                        .unwrap_or(result.token_in);

                Ok(QuoteResult {
                    steps: vec![],
                    total_amount_in_gross: result.token_in,
                    total_amount_in_net,
                    total_amount_out: result.token_est_out,
                    total_fee: result.trade_fee,
                    compute_units: ORCA_COMPUTE_UNITS,
                })
            }

            QuoteType::ExactOut(amount) => {
                let result = swap_quote_by_output_token(
                    amount,
                    ctx.a_to_b,
                    0,
                    whirlpool_facade,
                    oracle_facade,
                    tick_arrays,
                    timestamp,
                    transfer_fee_a,
                    transfer_fee_b,
                )
                .map_err(|e| anyhow::anyhow!("Orca ExactOut quote error: {e}"))?;

                let total_amount_in_net = try_apply_transfer_fee(
                    result.token_est_in,
                    transfer_fee_in.unwrap_or_default(),
                )
                .unwrap_or(result.token_est_in);

                Ok(QuoteResult {
                    steps: vec![],
                    total_amount_in_gross: result.token_est_in,
                    total_amount_in_net,
                    total_amount_out: result.token_out,
                    total_fee: result.trade_fee,
                    compute_units: ORCA_COMPUTE_UNITS,
                })
            }
        }
    }
}

impl DexMetrics for Whirlpool {
    fn dex_name(&self) -> &'static str {
        DEX_ORCA
    }
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct WhirlpoolRewardInfo {
    /// Reward token mint.
    pub mint: [u8; 32],
    /// Reward vault token account.
    pub vault: [u8; 32],
    /// reward_infos[0]: Authority account that has permission to initialize the reward and set
    /// emissions. reward_infos[1]: used for a struct that contains fields for extending the
    /// functionality of Whirlpool. reward_infos[2]: reserved for future use.
    ///
    /// Historical notes:
    /// Originally, this was a field named "authority", but it was found that there was no
    /// opportunity to set different authorities for the three rewards. Therefore, the use of
    /// this field was changed for Whirlpool's future extensibility.
    pub extension: [u8; 32],
    /// Q64.64 number that indicates how many tokens per second are earned per unit of liquidity.
    pub emissions_per_second_x64: [u64; 2],
    /// Q64.64 number that tracks the total tokens earned per unit of liquidity since the reward
    /// emissions were turned on.
    pub growth_global_x64: [u64; 2],
}

pub enum OrcaTickArray {
    Fixed(Box<FixedTickArray>),
    Dynamic(DynamicTickArray),
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct FixedTickArray {
    pub start_tick_index: i32,
    pub ticks_1: [Tick; 64],
    pub ticks_2: [Tick; 24],
    pub whirlpool: [u8; 32],
}

impl DexEntity for FixedTickArray {
    const PROGRAM_ID: Pubkey = ORCA_ID;
    const DISCRIMINATOR: &'static [u8] = &[69, 97, 189, 190, 110, 7, 66, 187];
    const DATA_SIZE: usize = 9988;

    fn deserialize(data: &[u8]) -> Option<Self> {
        Self::deserialize_bytemuck(data)
    }
}

impl FixedTickArray {
    #[must_use]
    pub fn pubkey(&self) -> Pubkey {
        Pubkey::from(self.whirlpool)
    }
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct Tick {
    // Total 113 bytes
    pub initialized: u8,           // 1
    pub liquidity_net: [u64; 2],   // 16
    pub liquidity_gross: [u64; 2], // 16

    // Q64.64
    pub fee_growth_outside_a: [u64; 2], // 16
    // Q64.64
    pub fee_growth_outside_b: [u64; 2], // 16

    // Array of Q64.64
    pub reward_growths_outside: [[u64; 2]; NUM_REWARDS], // 48 = 16 * 3
}

impl Tick {
    /// Check that the tick index is within the supported range of this contract
    ///
    /// # Parameters
    /// - `tick_index` - A i32 integer representing the tick index
    ///
    /// # Returns
    /// - `true`: The tick index is not within the range supported by this contract
    /// - `false`: The tick index is within the range supported by this contract
    #[must_use]
    pub fn check_is_out_of_bounds(tick_index: i32) -> bool {
        !(MIN_TICK_INDEX..=MAX_TICK_INDEX).contains(&tick_index)
    }

    /// Check that the tick index is a valid start tick for a tick array in this whirlpool
    /// A valid start-tick-index is a multiple of tick_spacing & number of ticks in a tick-array.
    ///
    /// # Parameters
    /// - `tick_index` - A i32 integer representing the tick index
    /// - `tick_spacing` - A u8 integer of the tick spacing for this whirlpool
    ///
    /// # Returns
    /// - `true`: The tick index is a valid start-tick-index for this whirlpool
    /// - `false`: The tick index is not a valid start-tick-index for this whirlpool or the tick
    ///   index not within the range supported by this contract
    #[must_use]
    pub fn check_is_valid_start_tick(tick_index: i32, tick_spacing: u16) -> bool {
        let ticks_in_array = TICK_ARRAY_SIZE as i32 * tick_spacing as i32;

        if Self::check_is_out_of_bounds(tick_index) {
            // Left-edge tick-array can have a start-tick-index smaller than the min tick index
            if tick_index > MIN_TICK_INDEX {
                return false;
            }

            let min_array_start_index =
                MIN_TICK_INDEX - (MIN_TICK_INDEX % ticks_in_array + ticks_in_array);
            return tick_index == min_array_start_index;
        }
        tick_index % ticks_in_array == 0
    }
}

#[derive(Debug, Clone)]
pub struct DynamicTickArray {
    pub start_tick_index: i32,
    pub whirlpool: Pubkey,
    pub tick_bitmap: u128,
    pub ticks: Vec<DynamicTickData>,
}

impl DexEntity for DynamicTickArray {
    const PROGRAM_ID: Pubkey = ORCA_ID;
    const DISCRIMINATOR: &'static [u8] = &[17, 216, 246, 142, 225, 199, 218, 56];
    const DATA_SIZE: usize = 0;

    fn deserialize(data: &[u8]) -> Option<Self> {
        // Validate overall data size (protection against corrupted or wrong accounts)
        if data.len() < Self::MIN_LEN || data.len() > Self::MAX_LEN {
            error!("Invalid DynamicTickArray size: {}", data.len());
            return None;
        }

        let disc_size = Self::DISCRIMINATOR.len();
        let mut offset = 0;

        // Verify discriminator
        if &data[offset..offset + disc_size] != Self::DISCRIMINATOR {
            error!("Discriminator mismatch");
            return None;
        }
        // Skip discriminator (already verified)
        offset += disc_size;

        let start_tick_index = i32::from_le_bytes(data.get(offset..offset + 4)?.try_into().ok()?);
        offset += 4;

        let whirlpool_bytes: [u8; 32] = data.get(offset..offset + 32)?.try_into().ok()?;
        let whirlpool = Pubkey::new_from_array(whirlpool_bytes);
        offset += 32;

        let bitmap_bytes: [u8; 16] = data.get(offset..offset + 16)?.try_into().ok()?;
        let tick_bitmap = u128::from_le_bytes(bitmap_bytes);
        offset += 16;

        let mut ticks = Vec::with_capacity(TICK_ARRAY_SIZE);

        for i in 0..TICK_ARRAY_SIZE {
            // Ensure at least 1 byte remains for discriminant
            if offset >= data.len() {
                error!("Unexpected end of data at tick {}", i);
                return None;
            }

            // Read discriminant (1 byte)
            let discriminant = data[offset];
            // Always advance offset after discriminant
            offset += 1;

            if discriminant == 1 {
                // Initialized tick
                if offset + DynamicTickData::LEN > data.len() {
                    error!("Not enough bytes for initialized tick {}", i);
                    return None;
                }

                let slice = &data[offset..offset + DynamicTickData::LEN];

                ticks.push(DynamicTickData {
                    liquidity_net: i128::from_le_bytes(slice[0..16].try_into().ok()?),
                    liquidity_gross: u128::from_le_bytes(slice[16..32].try_into().ok()?),
                    fee_growth_outside_a: u128::from_le_bytes(slice[32..48].try_into().ok()?),
                    fee_growth_outside_b: u128::from_le_bytes(slice[48..64].try_into().ok()?),
                    reward_growths_outside: [
                        u128::from_le_bytes(slice[64..80].try_into().ok()?),
                        u128::from_le_bytes(slice[80..96].try_into().ok()?),
                        u128::from_le_bytes(slice[96..112].try_into().ok()?),
                    ],
                });

                // Advance offset by the size of the parsed data
                offset += DynamicTickData::LEN;
            } else if discriminant == 0 {
                // Uninitialized — only 1 byte (discriminant) was read
                // No additional data to parse
            } else {
                // Invalid discriminant — corrupted data
                error!("Unknown discriminant {} at tick {}", discriminant, i);
                return None;
            }
        }

        Some(Self {
            start_tick_index,
            whirlpool,
            tick_bitmap,
            ticks,
        })
    }
}

impl DynamicTickArray {
    pub const MIN_LEN: usize = 148;
    pub const MAX_LEN: usize = 10004;

    #[must_use]
    pub fn pubkey(&self) -> Pubkey {
        self.whirlpool
    }
}

#[derive(Debug, Clone)]
pub struct DynamicTickData {
    pub liquidity_net: i128,
    pub liquidity_gross: u128,
    pub fee_growth_outside_a: u128,
    pub fee_growth_outside_b: u128,
    pub reward_growths_outside: [u128; 3],
}

impl DynamicTickData {
    pub const LEN: usize = 112;
}

#[allow(clippy::let_and_return)]
#[must_use]
pub fn get_start_tick_indexes(
    tick_current_index: i32,
    tick_spacing: u16,
    a_to_b: bool,
) -> Vec<i32> {
    let tick_spacing_i32 = tick_spacing as i32;
    let ticks_in_array = TICK_ARRAY_SIZE as i32 * tick_spacing_i32;

    let start_tick_index_base = floor_division(tick_current_index, ticks_in_array) * ticks_in_array;
    let offset = if a_to_b {
        [0, -1, -2]
    } else {
        let shifted =
            tick_current_index + tick_spacing_i32 >= start_tick_index_base + ticks_in_array;
        if shifted { [1, 2, 3] } else { [0, 1, 2] }
    };

    let start_tick_indexes = offset
        .iter()
        .filter_map(|&o| {
            let start_tick_index = start_tick_index_base + o * ticks_in_array;
            if Tick::check_is_valid_start_tick(start_tick_index, tick_spacing) {
                Some(start_tick_index)
            } else {
                None
            }
        })
        .collect::<Vec<i32>>();

    start_tick_indexes
}

// ---- From impls ----

impl From<&Whirlpool> for WhirlpoolFacade {
    fn from(w: &Whirlpool) -> Self {
        Self {
            fee_tier_index_seed: w.fee_tier_index_seed,
            tick_spacing: w.tick_spacing,
            fee_rate: w.fee_rate,
            protocol_fee_rate: w.protocol_fee_rate,
            liquidity: u128::from(w.liquidity[0]) | (u128::from(w.liquidity[1]) << 64),
            sqrt_price: u128::from(w.sqrt_price[0]) | (u128::from(w.sqrt_price[1]) << 64),
            tick_current_index: w.tick_current_index,
            fee_growth_global_a: u128::from(w.fee_growth_global_a[0])
                | (u128::from(w.fee_growth_global_a[1]) << 64),
            fee_growth_global_b: u128::from(w.fee_growth_global_b[0])
                | (u128::from(w.fee_growth_global_b[1]) << 64),
            reward_last_updated_timestamp: w.reward_last_updated_timestamp,
            reward_infos: w.reward_infos.map(|r| WhirlpoolRewardInfoFacade {
                emissions_per_second_x64: u128::from(r.emissions_per_second_x64[0])
                    | (u128::from(r.emissions_per_second_x64[1]) << 64),
                growth_global_x64: u128::from(r.growth_global_x64[0])
                    | (u128::from(r.growth_global_x64[1]) << 64),
            }),
        }
    }
}

impl From<&Tick> for TickFacade {
    fn from(tick: &Tick) -> Self {
        Self {
            initialized: tick.initialized != 0,
            liquidity_net: i128::from(tick.liquidity_net[0])
                | (i128::from(tick.liquidity_net[1]) << 64),
            liquidity_gross: u128::from(tick.liquidity_gross[0])
                | (u128::from(tick.liquidity_gross[1]) << 64),
            fee_growth_outside_a: u128::from(tick.fee_growth_outside_a[0])
                | (u128::from(tick.fee_growth_outside_a[1]) << 64),
            fee_growth_outside_b: u128::from(tick.fee_growth_outside_b[0])
                | (u128::from(tick.fee_growth_outside_b[1]) << 64),
            reward_growths_outside: [
                u128::from(tick.reward_growths_outside[0][0])
                    | (u128::from(tick.reward_growths_outside[0][1]) << 64),
                u128::from(tick.reward_growths_outside[1][0])
                    | (u128::from(tick.reward_growths_outside[1][1]) << 64),
                u128::from(tick.reward_growths_outside[2][0])
                    | (u128::from(tick.reward_growths_outside[2][1]) << 64),
            ],
        }
    }
}

impl From<&FixedTickArray> for TickArrayFacade {
    fn from(ta: &FixedTickArray) -> Self {
        let mut ticks = [TickFacade::default(); TICK_ARRAY_SIZE];
        for (i, tick) in ta.ticks_1.iter().enumerate() {
            ticks[i] = TickFacade::from(tick);
        }
        for (i, tick) in ta.ticks_2.iter().enumerate() {
            ticks[64 + i] = TickFacade::from(tick);
        }
        Self {
            start_tick_index: ta.start_tick_index,
            ticks,
        }
    }
}

impl From<&DynamicTickArray> for TickArrayFacade {
    fn from(ta: &DynamicTickArray) -> Self {
        let mut ticks = [TickFacade::default(); TICK_ARRAY_SIZE];
        let mut tick_idx = 0usize;
        for (offset, tick) in ticks.iter_mut().enumerate() {
            let initialized = (ta.tick_bitmap >> offset) & 1 == 1;
            if initialized {
                let data = &ta.ticks[tick_idx];
                *tick = TickFacade {
                    initialized: true,
                    liquidity_net: data.liquidity_net,
                    liquidity_gross: data.liquidity_gross,
                    fee_growth_outside_a: data.fee_growth_outside_a,
                    fee_growth_outside_b: data.fee_growth_outside_b,
                    reward_growths_outside: data.reward_growths_outside,
                };
                tick_idx += 1;
            }
        }
        Self {
            start_tick_index: ta.start_tick_index,
            ticks,
        }
    }
}
