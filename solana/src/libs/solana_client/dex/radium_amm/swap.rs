use bytemuck::{Pod, Zeroable};
use solana_sdk::pubkey::Pubkey;

use crate::libs::solana_client::{dex::radium_amm::constants::RAYDIUM_AMM_ID, registry::DexEntity};

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Swap {
    // todo
}

impl DexEntity for Swap {
    const PROGRAM_ID: Pubkey = RAYDIUM_AMM_ID;
    const DISCRIMINATOR: &'static [u8] = &[];
    const DATA_SIZE: usize = 0;

    fn deserialize(_data: &[u8]) -> Option<Self> {
        Some(Self {})
    }
}
