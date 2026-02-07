use bytemuck::{Pod, Zeroable};
use solana_sdk::pubkey::Pubkey;
use tracing::error;

use crate::libs::solana_client::{
    dex::orca::constants::{ORCA_ID, TICK_ARRAY_SIZE},
    registry::DexEntity,
};

pub const NUM_REWARDS: usize = 3;

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
    const DATA_SIZE: usize = 653;

    fn deserialize(data: &[u8]) -> Option<Self> {
        Self::deserialize_bytemuck(data)
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

#[derive(Debug, Clone)]
pub struct DynamicTickArray {
    pub start_tick_index: i32,
    pub whirlpool: Pubkey,
    pub tick_bitmap: u128,
    pub ticks: Vec<DynamicTickData>,
}

impl DynamicTickArray {
    pub const MIN_LEN: usize = 148;
    pub const MAX_LEN: usize = 10004;
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
