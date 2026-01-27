use crate::libs::solana_client::dex::{
    meteora_dlmm,
    models::{PoolState, TxEvent},
    radium_cpmm,
    registry::core::DexRegistry,
};

/// Populates the provided [DexRegistry] with protocol-specific parsers.
///
/// This function acts as a centralized configuration point for all supported
/// DEX integrations. It maps low-level protocol structs (e.g., Meteora or Raydium types)
/// to high-level domain wrappers like [PoolState] and [TxEvent].
///
/// ### Supported Protocols:
/// - **Meteora DLMM**: Pool state and Swap events.
/// - **Raydium CPMM**: Pool state and Swap events.
pub fn fill_registry(reg: &mut DexRegistry) {
    // Meteora DLMM Integration
    reg.add::<meteora_dlmm::LbPair, _>(PoolState::LbPairMeteoraDlmm);
    reg.add::<meteora_dlmm::Swap, _>(TxEvent::SwapMeteoraDlmm);

    // Raydium CPMM Integration
    reg.add::<radium_cpmm::PoolState, _>(PoolState::PoolStateRadiumCpmm);
    reg.add::<radium_cpmm::Swap, _>(TxEvent::SwapRadiumCpmm);
}
