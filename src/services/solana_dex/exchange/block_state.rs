use std::sync::{Arc, LazyLock};

use parking_lot::RwLock;
use solana_sdk::hash::Hash;

/// Global block state.
pub static BLOCK_STATE: LazyLock<Arc<RwLock<BlockState>>> =
    LazyLock::new(|| Arc::new(RwLock::new(BlockState::default())));

/// Struct for global block state.
#[derive(Debug, Clone, Copy, Default)]
pub struct BlockState {
    /// Current blockhash.
    pub blockhash: Hash,
    /// Last valid height for the blockhash.
    pub last_valid_height: u64,
}

/// Updates the global block state with new blockhash and last valid height.
///
/// Acquires a write lock on `BLOCK_STATE` and replaces the entire state atomically.
///
/// # Arguments
/// - `blockhash`: The new recent blockhash.
/// - `last_valid_height`: The last valid slot height for this blockhash.
#[inline]
pub fn update_block_state(blockhash: Hash, last_valid_height: u64) {
    *BLOCK_STATE.write() = BlockState {
        blockhash,
        last_valid_height,
    };
}

/// Returns a copy of the current global block state.
#[inline]
pub fn get_block_state() -> BlockState {
    *BLOCK_STATE.read()
}
