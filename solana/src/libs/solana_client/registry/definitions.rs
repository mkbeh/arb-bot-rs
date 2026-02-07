use crate::libs::solana_client::{
    dex::{meteora_damm_v2, meteora_dlmm, orca, pump_fun, radium_amm, radium_clmm, radium_cpmm},
    models::{PoolState, TxEvent},
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
/// - **Meteora DAMM V2**: Pool state and Swap events.
/// - **Raydium CPMM**: Pool state and Swap events.
/// - **Raydium AMM**: Pool state and Swap events.
/// - **Raydium CLMM**: Pool state and Swap events.
/// - **Orca**: Pool state and Swap events.
/// - **PumpFun**: Pool state and Swap events.
pub fn fill_registry(reg: &mut DexRegistry) {
    // Meteora DLMM Integration
    reg.add_boxed::<meteora_dlmm::LbPair, _>(PoolState::LbPairMeteoraDlmm);
    reg.add_boxed::<meteora_dlmm::BinArray, _>(PoolState::BinArrayMeteoraDlmm);
    reg.add::<meteora_dlmm::Swap, _>(TxEvent::SwapMeteoraDlmm);

    // Meteora DAMM V2 Integration
    reg.add_boxed::<meteora_damm_v2::Pool, _>(PoolState::PoolMeteoraDammV2);
    reg.add::<meteora_damm_v2::Swap, _>(TxEvent::SwapMeteoraDammV2);

    // Raydium CPMM Integration
    reg.add_boxed::<radium_cpmm::PoolState, _>(PoolState::PoolStateRadiumCpmm);
    reg.add::<radium_cpmm::Swap, _>(TxEvent::SwapRadiumCpmm);

    // Radium AMM Integration
    reg.add_boxed::<radium_amm::AmmInfo, _>(PoolState::AmmInfoRadiumAmm);
    reg.add::<radium_amm::Swap, _>(TxEvent::SwapRadiumAmm);

    // Radium CLMM Integration
    reg.add_boxed::<radium_clmm::PoolState, _>(PoolState::PoolStateRadiumClmm);
    reg.add_boxed::<radium_clmm::TickArrayState, _>(PoolState::TickArrayStateRadiumClmm);
    reg.add::<radium_clmm::Swap, _>(TxEvent::SwapRadiumClmm);

    // Orca Integration
    reg.add_boxed::<orca::Whirlpool, _>(PoolState::WhirlpoolOrca);
    reg.add_boxed::<orca::FixedTickArray, _>(PoolState::FixedTickArrayOrca);
    reg.add_boxed::<orca::DynamicTickArray, _>(PoolState::DynamicTickArrayOrca);
    reg.add::<orca::Swap, _>(TxEvent::SwapOrca);

    // PumpFun Integration
    reg.add_boxed::<pump_fun::BondingCurve, _>(PoolState::BondingCurvePumpFun);
    reg.add::<pump_fun::Swap, _>(TxEvent::SwapPumpFun);
}
