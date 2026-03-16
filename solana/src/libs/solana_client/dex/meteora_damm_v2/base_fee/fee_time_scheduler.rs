use bytemuck::{Pod, Zeroable};

use crate::libs::solana_client::dex::meteora_damm_v2::{
    CollectFeeMode,
    base_fee::BaseFeeHandler,
    error::PoolError,
    fee::*,
    math::{fee_math::*, safe_math::SafeMath},
    params::{TradeDirection, fee_parameters::validate_fee_fraction},
    state::BaseFeeMode,
    utils::activation_handler::ActivationType,
};

#[repr(C)]
#[derive(Default, Debug, Clone, Copy, Pod, Zeroable)]
pub struct PodAlignedFeeTimeScheduler {
    pub cliff_fee_numerator: u64,
    pub base_fee_mode: u8,
    pub padding: [u8; 5],
    pub number_of_period: u16,
    pub period_frequency: u64,
    pub reduction_factor: u64,
}

impl PodAlignedFeeTimeScheduler {
    pub fn get_max_base_fee_numerator(&self) -> u64 {
        self.cliff_fee_numerator
    }

    fn get_base_fee_numerator_by_period(&self, period: u64) -> anyhow::Result<u64> {
        let period = period.min(self.number_of_period.into());

        let base_fee_mode =
            BaseFeeMode::try_from(self.base_fee_mode).map_err(|_| PoolError::TypeCastFailed)?;

        match base_fee_mode {
            BaseFeeMode::FeeTimeSchedulerLinear => {
                let fee_numerator = self
                    .cliff_fee_numerator
                    .safe_sub(self.reduction_factor.safe_mul(period)?)?;
                Ok(fee_numerator)
            }
            BaseFeeMode::FeeTimeSchedulerExponential => {
                let period = u16::try_from(period).map_err(|_| PoolError::MathOverflow)?;
                let fee_numerator =
                    get_fee_in_period(self.cliff_fee_numerator, self.reduction_factor, period)?;
                Ok(fee_numerator)
            }
            _ => Err(PoolError::UndeterminedError.into()),
        }
    }

    pub fn get_base_fee_numerator(
        &self,
        current_point: u64,
        activation_point: u64,
    ) -> anyhow::Result<u64> {
        if self.period_frequency == 0 {
            return Ok(self.cliff_fee_numerator);
        }
        // it means alpha-vault is buying
        let period = if current_point < activation_point {
            self.number_of_period.into()
        } else {
            let period = current_point
                .safe_sub(activation_point)?
                .safe_div(self.period_frequency)?;
            period.min(self.number_of_period.into())
        };
        self.get_base_fee_numerator_by_period(period)
    }
}

impl BaseFeeHandler for PodAlignedFeeTimeScheduler {
    #[allow(clippy::collapsible_if)]
    fn validate(
        &self,
        _collect_fee_mode: CollectFeeMode,
        _activation_type: ActivationType,
    ) -> anyhow::Result<()> {
        if self.period_frequency != 0 || self.number_of_period != 0 || self.reduction_factor != 0 {
            if self.number_of_period == 0
                || self.period_frequency == 0
                || self.reduction_factor == 0
            {
                return Err(PoolError::InvalidFeeTimeScheduler.into());
            }
        }
        let min_fee_numerator = self.get_min_fee_numerator()?;
        let max_fee_numerator = self.get_max_fee_numerator()?;
        validate_fee_fraction(min_fee_numerator, FEE_DENOMINATOR)?;
        validate_fee_fraction(max_fee_numerator, FEE_DENOMINATOR)?;
        if min_fee_numerator < MIN_FEE_NUMERATOR
            || max_fee_numerator > get_max_fee_numerator(CURRENT_POOL_VERSION)?
        {
            return Err(PoolError::ExceedMaxFeeBps.into());
        }
        Ok(())
    }

    fn get_base_fee_numerator_from_included_fee_amount(
        &self,
        current_point: u64,
        activation_point: u64,
        _trade_direction: TradeDirection,
        _included_fee_amount: u64,
        _init_sqrt_price: u128,
        _current_sqrt_price: u128,
    ) -> anyhow::Result<u64> {
        self.get_base_fee_numerator(current_point, activation_point)
    }

    fn get_base_fee_numerator_from_excluded_fee_amount(
        &self,
        current_point: u64,
        activation_point: u64,
        _trade_direction: TradeDirection,
        _excluded_fee_amount: u64,
        _init_sqrt_price: u128,
        _current_sqrt_price: u128,
    ) -> anyhow::Result<u64> {
        self.get_base_fee_numerator(current_point, activation_point)
    }

    fn validate_base_fee_is_static(
        &self,
        current_point: u64,
        activation_point: u64,
    ) -> anyhow::Result<bool> {
        let scheduler_expiration_point = u128::from(activation_point)
            .safe_add(u128::from(self.number_of_period).safe_mul(self.period_frequency.into())?)?;
        Ok(u128::from(current_point) > scheduler_expiration_point)
    }

    fn get_min_fee_numerator(&self) -> anyhow::Result<u64> {
        self.get_base_fee_numerator_by_period(self.number_of_period.into())
    }

    fn get_max_fee_numerator(&self) -> anyhow::Result<u64> {
        Ok(self.get_max_base_fee_numerator())
    }
}
