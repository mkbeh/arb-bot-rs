use std::ops::Mul;

use bytemuck::{Pod, Zeroable};
use solana_sdk::pubkey::Pubkey;

use super::*;
use crate::libs::solana_client::registry::ProtocolEntity;

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct Reserve {
    pub version: u64,
    pub last_update: LastUpdate,
    pub lending_market: [u8; 32],
    pub farm_collateral: [u8; 32],
    pub farm_debt: [u8; 32],
    pub liquidity: ReserveLiquidity,
    pub reserve_liquidity_padding: [[u64; 30]; 5], // [u64; 150]
    pub collateral: ReserveCollateral,
    pub reserve_collateral_padding: [[u64; 30]; 5], // [u64; 150]
    pub config: ReserveConfig,
    pub config_padding: [[u64; 19]; 6], // [u64; 114]
    pub borrowed_amount_outside_elevation_group: u64,
    pub borrowed_amounts_against_this_reserve_in_elevation_groups: [u64; 32],
    pub withdraw_queue: WithdrawQueue,
    pub padding: [[u64; 17]; 12], // [u64; 204]
}

impl ProtocolEntity for Reserve {
    const PROGRAM_ID: Pubkey = KAMINO_ID;
    const DISCRIMINATOR: &'static [u8] = &[43, 242, 204, 202, 26, 247, 59, 127];
    const DATA_SIZE: usize = 8624;

    fn deserialize(data: &[u8]) -> Option<Self> {
        Self::deserialize_bytemuck(data)
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct LastUpdate {
    slot: u64,
    stale: u8,
    price_status: u8,
    placeholder: [u8; 6],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct ReserveLiquidity {
    pub mint_pubkey: [u8; 32],
    pub supply_vault: [u8; 32],
    pub fee_vault: [u8; 32],
    pub total_available_amount: u64,
    pub borrowed_amount_sf: [u64; 2],
    pub market_price_sf: [u64; 2],
    pub market_price_last_updated_ts: u64,
    pub mint_decimals: u64,
    pub deposit_limit_crossed_timestamp: u64,
    pub borrow_limit_crossed_timestamp: u64,
    pub cumulative_borrow_rate_bsf: BigFractionBytes,
    pub accumulated_protocol_fees_sf: [u64; 2],
    pub accumulated_referrer_fees_sf: [u64; 2],
    pub pending_referrer_fees_sf: [u64; 2],
    pub absolute_referral_rate_sf: [u64; 2],
    pub token_program: [u8; 32],
    pub padding2: [[u64; 17]; 3], // [u64; 51]
    pub padding3: [u8; 512],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct BigFractionBytes {
    pub value: [u64; 4],
    pub padding: [u64; 2],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct ReserveCollateral {
    pub mint_pubkey: [u8; 32],
    pub mint_total_supply: u64,
    pub supply_vault: [u8; 32],
    pub _padding1: [u8; 512],
    pub _padding2: [u8; 512],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct ReserveConfig {
    pub status: u8,
    pub padding_deprecated_asset_tier: u8,
    pub host_fixed_interest_rate_bps: u16,
    pub min_deleveraging_bonus_bps: u16,
    pub block_ctoken_usage: u8,
    pub reserved_1: [u8; 6],
    pub protocol_order_execution_fee_pct: u8,
    pub protocol_take_rate_pct: u8,
    pub protocol_liquidation_fee_pct: u8,
    pub loan_to_value_pct: u8,
    pub liquidation_threshold_pct: u8,
    pub min_liquidation_bonus_bps: u16,
    pub max_liquidation_bonus_bps: u16,
    pub bad_debt_liquidation_bonus_bps: u16,
    pub deleveraging_margin_call_period_secs: u64,
    pub deleveraging_threshold_decrease_bps_per_day: u64,
    pub fees: ReserveFees,
    pub borrow_rate_curve: BorrowRateCurve,
    pub borrow_factor_pct: u64,
    pub deposit_limit: u64,
    pub borrow_limit: u64,
    pub token_info: TokenInfo,
    pub deposit_withdrawal_cap: WithdrawalCaps,
    pub debt_withdrawal_cap: WithdrawalCaps,
    pub elevation_groups: [u8; 20],
    pub disable_usage_as_coll_outside_emode: u8,
    pub utilization_limit_block_borrowing_above_pct: u8,
    pub autodeleverage_enabled: u8,
    pub proposer_authority_locked: u8,
    pub borrow_limit_outside_elevation_group: u64,
    pub borrow_limit_against_this_collateral_in_elevation_group: [u64; 32],
    pub deleveraging_bonus_increase_bps_per_day: u64,
    pub debt_maturity_timestamp: u64,
    pub debt_term_seconds: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct ReserveFees {
    pub origination_fee_sf: u64,
    pub flash_loan_fee_sf: u64,
    pub padding: [u8; 8],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct BorrowRateCurve {
    pub points: [CurvePoint; 11],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct CurvePoint {
    pub utilization_rate_bps: u32,
    pub borrow_rate_bps: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct TokenInfo {
    pub name: [u8; 32],
    pub heuristic: PriceHeuristic,
    pub max_twap_divergence_bps: u64,
    pub max_age_price_seconds: u64,
    pub max_age_twap_seconds: u64,
    pub scope_configuration: ScopeConfiguration,
    pub switchboard_configuration: SwitchboardConfiguration,
    pub pyth_configuration: PythConfiguration,
    pub block_price_usage: u8,
    pub reserved: [u8; 7],
    pub _padding: [u64; 19],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct WithdrawQueue {
    pub queued_collateral_amount: u64,
    pub next_issued_ticket_sequence_number: u64,
    pub next_withdrawable_ticket_sequence_number: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct WithdrawalCaps {
    pub config_capacity: i64,
    pub current_total: i64,
    pub last_interval_start_timestamp: u64,
    pub config_interval_length_seconds: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct PriceHeuristic {
    pub lower: u64,
    pub upper: u64,
    pub exp: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct ScopeConfiguration {
    pub price_feed: [u8; 32],
    pub price_chain: [u16; 4],
    pub twap_chain: [u16; 4],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct SwitchboardConfiguration {
    pub price_aggregator: [u8; 32],
    pub twap_aggregator: [u8; 32],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct PythConfiguration {
    pub price: [u8; 32],
}

#[must_use]
pub fn calculate_flash_loan_repay_amount(flash_loan_fee_sf: u64, amount: u64) -> u64 {
    let amount_f = Fraction::from_num(amount);
    let fee = calculate_flash_loan_fee(flash_loan_fee_sf, amount_f);
    amount.saturating_add(fee)
}

#[must_use]
pub fn calculate_flash_loan_fee(flash_loan_fee_sf: u64, amount: Fraction) -> u64 {
    let fee_rate = Fraction::from_bits(flash_loan_fee_sf.into());

    if fee_rate == Fraction::ZERO || amount == Fraction::ZERO {
        return 0;
    }

    let minimum_fee = 1u64;
    let fee_f = amount.mul(fee_rate);
    let fee_f = fee_f.max(Fraction::from_num(minimum_fee));

    fee_f.to_round()
}
