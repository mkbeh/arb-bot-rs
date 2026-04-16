use solana_sdk::pubkey::Pubkey;

use crate::libs::solana_client::protocols::meteora_dlmm::METEORA_DLMM_ID;

pub const BIN_ARRAY_BITMAP_SEED: &[u8] = b"bitmap";

#[must_use]
pub fn derive_bin_array_bitmap_extension(lb_pair: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[BIN_ARRAY_BITMAP_SEED, lb_pair.as_ref()], &METEORA_DLMM_ID).0
}
