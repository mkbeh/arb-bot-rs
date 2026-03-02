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

    fn quote(
        &self,
        ctx: &QuoteContext,
        data: Option<&LiquidityMap>,
    ) -> anyhow::Result<QuoteResult, QuoteError>;
}

pub struct QuoteResult {
    pub steps: Vec<SwapResult>,
    pub total_amount_out: u64,
    pub compute_units: i32,
}

pub struct SwapResult {
    pub pool_state_id: i32,
    pub amount_in: u64,
    pub amount_out: u64,
    pub price: u64,
}

pub struct TokenConfig {
    pub transfer_fee_bps: u16,
    pub max_transfer_fee: u64,
}

pub enum QuoteType {
    ExactIn(u64),
    ExactOut(u64),
}

pub struct QuoteContext {
    pub quote_type: QuoteType,
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
