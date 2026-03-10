use bytemuck::{Pod, Zeroable};
use solana_sdk::pubkey::Pubkey;

use crate::libs::solana_client::{
    dex::raydium_cpmm::constants::*, metrics::*, pool::*, registry::DexEntity,
};

/// Holds the current owner of the factory
#[repr(C, packed)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct AmmConfig {
    /// Bump to identify PDA
    pub bump: u8,
    /// Status to control if new pool can be create
    pub disable_create_pool: u8,
    /// Config index
    pub index: u16,
    /// The trade fee, denominated in hundredths of a bip (10^-6)
    pub trade_fee_rate: u64,
    /// The protocol fee
    pub protocol_fee_rate: u64,
    /// The fund fee, denominated in hundredths of a bip (10^-6)
    pub fund_fee_rate: u64,
    /// Fee for create a new pool
    pub create_pool_fee: u64,
    /// Address of the protocol fee owner
    pub protocol_owner: [u8; 32],
    /// Address of the fund fee owner
    pub fund_owner: [u8; 32],
    /// The pool creator fee, denominated in hundredths of a bip (10^-6)
    pub creator_fee_rate: u64,
    /// padding
    pub padding: [u64; 15],
}

impl DexEntity for AmmConfig {
    const PROGRAM_ID: Pubkey = RAYDIUM_CPMM_ID;
    const DISCRIMINATOR: &'static [u8] = &[218, 244, 33, 104, 203, 203, 43, 111];
    const DATA_SIZE: usize = 8 + 1 + 1 + 2 + 4 * 8 + 32 * 2 + 8 + 8 * 15; // 236

    fn deserialize(data: &[u8]) -> Option<Self> {
        Self::deserialize_bytemuck(data)
    }
}

#[repr(C, packed)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct PoolState {
    pub amm_config: [u8; 32],
    pub pool_creator: [u8; 32],
    pub token_0_vault: [u8; 32],
    pub token_1_vault: [u8; 32],
    pub lp_mint: [u8; 32],
    pub token_0_mint: [u8; 32],
    pub token_1_mint: [u8; 32],
    pub token_0_program: [u8; 32],
    pub token_1_program: [u8; 32],
    pub observation_key: [u8; 32],

    pub auth_bump: u8,
    pub status: u8,
    pub lp_mint_decimals: u8,
    pub mint_0_decimals: u8,
    pub mint_1_decimals: u8,

    pub lp_supply: u64,
    pub protocol_fees_token_0: u64,
    pub protocol_fees_token_1: u64,
    pub fund_fees_token_0: u64,
    pub fund_fees_token_1: u64,
    pub open_time: u64,
    pub recent_epoch: u64,

    pub padding: [u64; 31],
}

impl DexEntity for PoolState {
    const PROGRAM_ID: Pubkey = RAYDIUM_CPMM_ID;
    const DISCRIMINATOR: &'static [u8] = &[247, 237, 227, 245, 215, 195, 222, 70];
    const DATA_SIZE: usize = 637;

    fn deserialize(data: &[u8]) -> Option<Self> {
        Self::deserialize_bytemuck(data)
    }
}

impl DexPool for PoolState {
    fn get_mint_a(&self) -> Pubkey {
        Pubkey::from(self.token_0_mint)
    }

    fn get_mint_b(&self) -> Pubkey {
        Pubkey::from(self.token_1_mint)
    }

    #[allow(clippy::todo)]
    fn quote(&self, _ctx: &QuoteContext) -> anyhow::Result<QuoteResult> {
        todo!()
    }
}

impl DexMetrics for PoolState {
    fn dex_name(&self) -> &'static str {
        DEX_RAYDIUM_CPMM
    }
}
