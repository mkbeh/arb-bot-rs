use std::collections::BTreeMap;

use crate::libs::solana_client::dex::{meteora_dlmm, orca, raydium_clmm};

/// Protocol-specific bitmap extension for locating liquidity arrays.
///
/// Used during swap simulation to skip empty bin/tick arrays efficiently
/// without iterating over each one individually.
pub enum LiquidityBitmap<'a> {
    MeteoraDlmm(Option<&'a meteora_dlmm::BinArrayBitmapExtension>),
    RaydiumClmm(Option<&'a raydium_clmm::TickArrayBitmapExtension>),
}

/// A single liquidity array from any supported DEX protocol.
pub enum LiquidityArray {
    MeteoraDlmm(meteora_dlmm::BinArray),
    OrcaFixed(orca::FixedTickArray),
    OrcaDynamic(orca::DynamicTickArray),
    RaydiumClmm(raydium_clmm::TickArrayState),
}

/// A reference to a sorted collection of liquidity arrays for a specific DEX protocol.
pub enum LiquidityMap<'a> {
    MeteoraDlmm(&'a BTreeMap<i64, meteora_dlmm::BinArray>),
    RaydiumClmm(&'a BTreeMap<i32, raydium_clmm::TickArrayState>),
    OrcaFixed(&'a BTreeMap<i32, orca::FixedTickArray>),
    OrcaDynamic(&'a BTreeMap<i32, orca::DynamicTickArray>),
}

/// Converts a protocol-specific liquidity array type into a [`LiquidityMap`] variant.
///
/// Implemented for each array type to allow the liquidity cache to wrap
/// its internal `BTreeMap` into the appropriate `LiquidityMap` variant
/// without knowing the concrete type at the call site.
pub trait IntoLiquidityMap<'a>: Sized {
    /// The key type used in the underlying `BTreeMap`.
    type Key: Ord;

    /// Wraps a reference to a sorted map of liquidity arrays into a [`LiquidityMap`].
    fn wrap_to_map(map: &'a BTreeMap<Self::Key, Self>) -> LiquidityMap<'a>;
}

// --- Liquidity Map Implementations ---

impl<'a> IntoLiquidityMap<'a> for meteora_dlmm::BinArray {
    type Key = i64;
    fn wrap_to_map(map: &'a BTreeMap<Self::Key, Self>) -> LiquidityMap<'a> {
        LiquidityMap::MeteoraDlmm(map)
    }
}

impl<'a> IntoLiquidityMap<'a> for raydium_clmm::TickArrayState {
    type Key = i32;
    fn wrap_to_map(map: &'a BTreeMap<Self::Key, Self>) -> LiquidityMap<'a> {
        LiquidityMap::RaydiumClmm(map)
    }
}

impl<'a> IntoLiquidityMap<'a> for orca::FixedTickArray {
    type Key = i32;
    fn wrap_to_map(map: &'a BTreeMap<Self::Key, Self>) -> LiquidityMap<'a> {
        LiquidityMap::OrcaFixed(map)
    }
}

impl<'a> IntoLiquidityMap<'a> for orca::DynamicTickArray {
    type Key = i32;
    fn wrap_to_map(map: &'a BTreeMap<Self::Key, Self>) -> LiquidityMap<'a> {
        LiquidityMap::OrcaDynamic(map)
    }
}
