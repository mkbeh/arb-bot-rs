#![allow(clippy::match_same_arms)]

use num_enum::{IntoPrimitive, TryFromPrimitive};

use crate::libs::solana_client::dex::meteora_damm_v2::{CollectFeeMode, params::TradeDirection};

#[derive(Debug, PartialEq)]
pub struct FeeOnAmountResult {
    pub amount: u64,
    pub claiming_fee: u64,
    pub compounding_fee: u64,
    pub protocol_fee: u64,
    pub referral_fee: u64,
}

/// collect fee mode
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, IntoPrimitive, TryFromPrimitive)]
// https://www.desmos.com/calculator/oxdndn2xdx
pub enum BaseFeeMode {
    // fee = cliff_fee_numerator - passed_period * reduction_factor
    // passed_period = (current_point - activation_point) / period_frequency
    FeeTimeSchedulerLinear,
    // fee = cliff_fee_numerator * (1-reduction_factor/10_000)^passed_period
    FeeTimeSchedulerExponential,
    // rate limiter
    RateLimiter,
    // fee = cliff_fee_numerator - passed_period * reduction_factor
    // passed_period = changed_price / sqrt_price_step_bps
    // passed_period = (current_sqrt_price - init_sqrt_price) * 10_000 / init_sqrt_price /
    // sqrt_price_step_bps
    FeeMarketCapSchedulerLinear,
    // fee = cliff_fee_numerator * (1-reduction_factor/10_000)^passed_period
    FeeMarketCapSchedulerExponential,
}

#[derive(Default, Debug)]
pub struct FeeMode {
    pub fees_on_input: bool,
    pub fees_on_token_a: bool,
    pub has_referral: bool,
}

impl FeeMode {
    pub fn get_fee_mode(
        collect_fee_mode: CollectFeeMode,
        trade_direction: TradeDirection,
        has_referral: bool,
    ) -> Self {
        let (fees_on_input, fees_on_token_a) = match (collect_fee_mode, trade_direction) {
            // When collecting fees on output token
            (CollectFeeMode::BothToken, TradeDirection::AtoB) => (false, false),
            (CollectFeeMode::BothToken, TradeDirection::BtoA) => (false, true),

            // When collecting fees on tokenB
            (CollectFeeMode::OnlyB, TradeDirection::AtoB) => (false, false),
            (CollectFeeMode::OnlyB, TradeDirection::BtoA) => (true, false),

            // when collecting fees on compounding
            (CollectFeeMode::Compounding, TradeDirection::AtoB) => (false, false),
            (CollectFeeMode::Compounding, TradeDirection::BtoA) => (true, false),
        };

        Self {
            fees_on_input,
            fees_on_token_a,
            has_referral,
        }
    }
}

pub struct SplitFees {
    pub claiming_fee: u64,
    pub compounding_fee: u64,
    pub protocol_fee: u64,
    pub referral_fee: u64,
}
