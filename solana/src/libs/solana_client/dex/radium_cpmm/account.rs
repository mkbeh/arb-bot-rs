use bytemuck::{Pod, Zeroable};
use solana_sdk::pubkey::Pubkey;

use crate::libs::solana_client::dex::{
    radium_cpmm::constants::RADIUM_CPMM_ID, registry::DexEntity,
};

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
    const PROGRAM_ID: Pubkey = RADIUM_CPMM_ID;
    const DISCRIMINATOR: &'static [u8] = &[247, 237, 227, 245, 215, 195, 222, 70];
    const POOL_SIZE: usize = 637;

    fn deserialize(data: &[u8]) -> Option<Self> {
        Self::deserialize_bytemuck(data)
    }
}
