use bytemuck::{Pod, Zeroable};
use solana_sdk::pubkey::Pubkey;

use crate::libs::solana_client::{dex::pump_fun::constants::PUMP_FUN_ID, registry::DexEntity};

#[repr(C, packed)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct BondingCurve {
    pub virtual_token_reserves: u64,
    pub virtual_sol_reserves: u64,
    pub real_token_reserves: u64,
    pub real_sol_reserves: u64,
    pub token_total_supply: u64,
    pub complete: u8,
    pub _padding: u8,
    pub creator: [u8; 32],
}

impl DexEntity for BondingCurve {
    const PROGRAM_ID: Pubkey = PUMP_FUN_ID;
    const DISCRIMINATOR: &'static [u8] = &[23, 183, 248, 55, 96, 216, 172, 96];
    const POOL_SIZE: usize = 82;

    fn deserialize(data: &[u8]) -> Option<Self> {
        Self::deserialize_bytemuck(data)
    }
}
