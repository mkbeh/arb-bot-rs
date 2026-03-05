use std::collections::BTreeMap;

use solana_sdk::{account::Account, clock::Clock, pubkey::Pubkey};

use crate::libs::solana_client::{
    dex::{meteora_dlmm, orca, raydium_clmm},
    metrics::DexMetrics,
};

pub trait DexPool: DexMetrics + Send + Sync {
    fn get_mint_a(&self) -> Pubkey;

    fn get_mint_b(&self) -> Pubkey;

    fn get_mints(&self) -> (Pubkey, Pubkey) {
        (self.get_mint_a(), self.get_mint_b())
    }

    fn quote(&self, ctx: &QuoteContext, data: Option<&LiquidityMap>)
    -> anyhow::Result<QuoteResult>;
}

/// Result of a swap simulation (quote) for arbitrage calculations.
pub struct QuoteResult {
    /// Detailed step-by-step breakdown of each bin crossed during the swap.
    pub steps: Vec<QuoteSwapResult>,

    /// The actual gross amount to be deducted from the wallet.
    pub total_amount_in_gross: u64,

    /// The net amount that effectively entered the pool's bin arrays after network fees.
    pub total_amount_in_net: u64,

    /// The final amount received in the destination wallet.
    pub total_amount_out: u64,

    /// Total swap fees paid to liquidity providers and the protocol (LP fee + Protocol fee).
    /// These fees are already deducted from the `total_amount_out` during simulation.
    pub total_fee: u64,

    /// Estimated Solana Compute Units (CU) required for the swap transaction.
    pub compute_units: u32,
}

pub struct QuoteSwapResult {
    pub pool_state_id: i32,
    pub amount_in: u64,
    pub amount_out: u64,
    pub fee: u64,
    pub price: u128,
}

pub enum QuoteType {
    ExactIn(u64),
    ExactOut(u64),
}

pub struct QuoteContext<'a> {
    pub quote_type: QuoteType,
    pub a_to_b: bool,
    pub clock: &'a Clock,
    pub mint_in: &'a Account,
    pub mint_out: &'a Account,
    pub bitmap: Option<LiquidityBitmap<'a>>,
}

pub enum LiquidityBitmap<'a> {
    MeteoraDlmm(Option<&'a meteora_dlmm::BinArrayBitmapExtension>),
    RaydiumClmm(Option<&'a raydium_clmm::TickArrayBitmapExtension>),
}

pub enum LiquidityArray {
    MeteoraDlmm(meteora_dlmm::BinArray),
    OrcaFixed(orca::FixedTickArray),
    OrcaDynamic(orca::DynamicTickArray),
    RaydiumClmm(raydium_clmm::TickArrayState),
}

pub enum LiquidityMap<'a> {
    MeteoraDlmm(&'a BTreeMap<i64, meteora_dlmm::BinArray>),
    RaydiumClmm(&'a BTreeMap<i32, raydium_clmm::TickArrayState>),
    OrcaFixed(&'a BTreeMap<i32, orca::FixedTickArray>),
    OrcaDynamic(&'a BTreeMap<i32, orca::DynamicTickArray>),
}

pub trait IntoLiquidityMap<'a>: Sized {
    type Key: Ord;
    fn wrap_to_map(map: &'a BTreeMap<Self::Key, Self>) -> LiquidityMap<'a>;
}

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
