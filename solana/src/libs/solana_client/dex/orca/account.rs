use bytemuck::{Pod, Zeroable};
use solana_sdk::pubkey::Pubkey;

use crate::libs::solana_client::{dex::orca::constants::ORCA_ID, registry::DexEntity};

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
    const POOL_SIZE: usize = 653;

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
