use bytemuck::{Pod, Zeroable};
use solana_sdk::pubkey::Pubkey;

use crate::libs::solana_client::dex::{parser::DexEntity, radium_cpmm::constants::RADIUM_CPMM_ID};

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Swap {
    // todo
}

impl DexEntity for Swap {
    const PROGRAM_ID: Pubkey = RADIUM_CPMM_ID;
    const DISCRIMINATOR: &'static [u8] = &[];
    const POOL_SIZE: usize = 0;

    fn deserialize(_data: &[u8]) -> Option<Self> {
        Some(Swap {})
    }
}
