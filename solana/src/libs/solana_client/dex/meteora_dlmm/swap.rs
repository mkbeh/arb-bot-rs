use bytemuck::{Pod, Zeroable};
use solana_sdk::pubkey::Pubkey;

use crate::libs::solana_client::{
    dex::meteora_dlmm::constants::METEORA_DLMM_ID, registry::ProtocolEntity,
};

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Swap {
    // todo
}

impl ProtocolEntity for Swap {
    const PROGRAM_ID: Pubkey = METEORA_DLMM_ID;
    const DISCRIMINATOR: &'static [u8] = &[];
    const DATA_SIZE: usize = 0;

    fn deserialize(_data: &[u8]) -> Option<Self> {
        Some(Self {})
    }
}
