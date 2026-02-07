use bytemuck::{Pod, Zeroable};
use solana_sdk::pubkey::Pubkey;

use crate::libs::solana_client::{dex::pump_fun::constants::PUMP_FUN_ID, registry::DexEntity};

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Swap {
    // todo
}

impl DexEntity for Swap {
    const PROGRAM_ID: Pubkey = PUMP_FUN_ID;
    const DISCRIMINATOR: &'static [u8] = &[];
    const DATA_SIZE: usize = 0;

    fn deserialize(_data: &[u8]) -> Option<Self> {
        Some(Self {})
    }
}
