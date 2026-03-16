#![allow(dead_code)]

pub mod base_fee_serde;
pub mod fee_market_cap_scheduler;
pub mod fee_rate_limiter;
pub mod fee_time_scheduler;

pub use base_fee_serde::*;

use crate::libs::solana_client::dex::meteora_damm_v2::{
    CollectFeeMode, params::TradeDirection, utils::activation_handler::ActivationType,
};

pub trait BaseFeeHandler {
    fn validate(
        &self,
        collect_fee_mode: CollectFeeMode,
        activation_type: ActivationType,
    ) -> anyhow::Result<()>;
    fn get_base_fee_numerator_from_included_fee_amount(
        &self,
        current_point: u64,
        activation_point: u64,
        trade_direction: TradeDirection,
        included_fee_amount: u64,
        init_sqrt_price: u128,
        current_sqrt_price: u128,
    ) -> anyhow::Result<u64>;
    fn get_base_fee_numerator_from_excluded_fee_amount(
        &self,
        current_point: u64,
        activation_point: u64,
        trade_direction: TradeDirection,
        excluded_fee_amount: u64,
        init_sqrt_price: u128,
        current_sqrt_price: u128,
    ) -> anyhow::Result<u64>;

    fn validate_base_fee_is_static(
        &self,
        current_point: u64,
        activation_point: u64,
    ) -> anyhow::Result<bool>;

    fn get_min_fee_numerator(&self) -> anyhow::Result<u64>;

    fn get_max_fee_numerator(&self) -> anyhow::Result<u64>;
}
