use bytemuck::{Pod, Zeroable};
use solana_sdk::pubkey::Pubkey;

use crate::libs::solana_client::{
    dex::meteora_damm_v2::constants::METEORA_DAMM_V2_ID, registry::DexEntity,
};

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Swap {
    // todo
}

impl DexEntity for Swap {
    const PROGRAM_ID: Pubkey = METEORA_DAMM_V2_ID;
    const DISCRIMINATOR: &'static [u8] = &[];
    const DATA_SIZE: usize = 0;

    fn deserialize(_data: &[u8]) -> Option<Self> {
        Some(Self {})
    }
}
