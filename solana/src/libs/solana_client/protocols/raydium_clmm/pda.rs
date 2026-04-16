use solana_sdk::pubkey::Pubkey;

use crate::libs::solana_client::protocols::raydium_clmm::RAYDIUM_CLMM_ID;

pub const POOL_TICK_ARRAY_BITMAP_SEED: &str = "pool_tick_array_bitmap_extension";

#[must_use]
pub fn derive_tick_array_bitmap_extension(pool_id: Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[POOL_TICK_ARRAY_BITMAP_SEED.as_bytes(), pool_id.as_ref()],
        &RAYDIUM_CLMM_ID,
    )
    .0
}
