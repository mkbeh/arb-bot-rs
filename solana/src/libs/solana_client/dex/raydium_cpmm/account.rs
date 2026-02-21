use bytemuck::{Pod, Zeroable};
use solana_sdk::pubkey::Pubkey;

use crate::libs::solana_client::{
    dex::raydium_cpmm::constants::RAYDIUM_CPMM_ID,
    metrics::{DEX_RAYDIUM_CPMM, DexMetrics},
    pool::{
        DexPool,
        traits::{LiquidityMap, MultiQuote, QuoteContext, QuoteError},
    },
    registry::DexEntity,
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

    fn quote_out(
        &self,
        amount_in: u64,
        ctx: &QuoteContext,
        data: &LiquidityMap,
    ) -> anyhow::Result<MultiQuote, QuoteError> {
        todo!()
    }
}

impl DexMetrics for PoolState {
    fn dex_name(&self) -> &'static str {
        DEX_RAYDIUM_CPMM
    }
}
