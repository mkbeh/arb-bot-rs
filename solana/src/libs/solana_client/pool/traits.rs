use std::collections::BTreeMap;

use solana_sdk::pubkey::Pubkey;

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

    fn quote_out(
        &self,
        amount_in: u64,
        ctx: &QuoteContext,
        data: &LiquidityMap,
    ) -> anyhow::Result<MultiQuote, QuoteError>;
}

pub struct MultiQuote {
    pub steps: Vec<SwapStep>,
    pub total_amount_out: u64,
}

pub struct SwapStep {
    pub bin_id: i32,
    pub amount_in: u64,
    pub amount_out: u64,
    pub lp_fee: u64,
    pub protocol_fee: u64,
    pub token_tax_out: u64,
    pub next_price: f64,
}

pub struct TokenConfig {
    pub transfer_fee_bps: u16,
    pub max_transfer_fee: u64,
}

pub struct QuoteContext {
    pub a_to_b: bool,
    pub token_in_config: TokenConfig,
    pub token_out_config: TokenConfig,
}

#[derive(Debug, Copy, Clone)]
pub enum QuoteError {
    InsufficientLiquidity,
    InvalidPoolState,
    Overflow,
    BaseTokenTaxTooHigh,
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
    None, // For standard AMM
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
