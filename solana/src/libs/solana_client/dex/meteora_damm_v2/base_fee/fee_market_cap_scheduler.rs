use bytemuck::{Pod, Zeroable};
use ruint::aliases::U256;

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
pub struct PodAlignedFeeMarketCapScheduler {
    pub cliff_fee_numerator: u64,
    pub base_fee_mode: u8,
    pub padding: [u8; 5],
    pub number_of_period: u16,
    pub sqrt_price_step_bps: u32,
    pub scheduler_expiration_duration: u32,
    pub reduction_factor: u64,
}

impl PodAlignedFeeMarketCapScheduler {
    fn get_base_fee_numerator_by_period(&self, period: u64) -> anyhow::Result<u64> {
        let period = period.min(self.number_of_period.into());

        let base_fee_mode =
            BaseFeeMode::try_from(self.base_fee_mode).map_err(|_| PoolError::TypeCastFailed)?;

        match base_fee_mode {
            BaseFeeMode::FeeMarketCapSchedulerLinear => {
                let fee_numerator = self
                    .cliff_fee_numerator
                    .safe_sub(self.reduction_factor.safe_mul(period)?)?;
                Ok(fee_numerator)
            }
            BaseFeeMode::FeeMarketCapSchedulerExponential => {
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
        init_sqrt_price: u128,
        current_sqrt_price: u128,
    ) -> anyhow::Result<u64> {
        let scheduler_expiration_point =
            activation_point.safe_add(self.scheduler_expiration_duration.into())?;

        let period =
            if current_point > scheduler_expiration_point || current_point < activation_point {
                // Expired or alpha vault is buying
                self.number_of_period.into()
            } else {
                let period = if current_sqrt_price <= init_sqrt_price {
                    0u64
                } else {
                    let current_sqrt_price = U256::from(current_sqrt_price);
                    let init_sqrt_price = U256::from(init_sqrt_price);
                    let max_bps = U256::from(MAX_BASIS_POINT);
                    let sqrt_price_step_bps = U256::from(self.sqrt_price_step_bps);
                    let passed_period = current_sqrt_price
                        .safe_sub(init_sqrt_price)?
                        .safe_mul(max_bps)?
                        .safe_div(init_sqrt_price)?
                        .safe_div(sqrt_price_step_bps)?;

                    if passed_period > U256::from(self.number_of_period) {
                        self.number_of_period.into()
                    } else {
                        // that should never return error
                        passed_period
                            .try_into()
                            .map_err(|_| PoolError::UndeterminedError)?
                    }
                };
                period.min(self.number_of_period.into())
            };
        self.get_base_fee_numerator_by_period(period)
    }
}

impl BaseFeeHandler for PodAlignedFeeMarketCapScheduler {
    fn validate(
        &self,
        _collect_fee_mode: CollectFeeMode,
        _activation_type: ActivationType,
    ) -> anyhow::Result<()> {
        // doesn't allow zero fee marketcap scheduler
        if self.reduction_factor == 0 {
            return Err(PoolError::InvalidFeeMarketCapScheduler.into());
        }
        if self.sqrt_price_step_bps == 0 {
            return Err(PoolError::InvalidFeeMarketCapScheduler.into());
        }
        if self.scheduler_expiration_duration == 0 {
            return Err(PoolError::InvalidFeeMarketCapScheduler.into());
        }
        if self.number_of_period == 0 {
            return Err(PoolError::InvalidFeeMarketCapScheduler.into());
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

    fn get_base_fee_numerator_from_excluded_fee_amount(
        &self,
        current_point: u64,
        activation_point: u64,
        _trade_direction: TradeDirection,
        _excluded_fee_amount: u64,
        init_sqrt_price: u128,
        current_sqrt_price: u128,
    ) -> anyhow::Result<u64> {
        self.get_base_fee_numerator(
            current_point,
            activation_point,
            init_sqrt_price,
            current_sqrt_price,
        )
    }

    fn get_base_fee_numerator_from_included_fee_amount(
        &self,
        current_point: u64,
        activation_point: u64,
        _trade_direction: TradeDirection,
        _included_fee_amount: u64,
        init_sqrt_price: u128,
        current_sqrt_price: u128,
    ) -> anyhow::Result<u64> {
        self.get_base_fee_numerator(
            current_point,
            activation_point,
            init_sqrt_price,
            current_sqrt_price,
        )
    }

    fn validate_base_fee_is_static(
        &self,
        current_point: u64,
        activation_point: u64,
    ) -> anyhow::Result<bool> {
        let scheduler_expiration_point =
            u128::from(activation_point).safe_add(self.scheduler_expiration_duration.into())?;
        Ok(u128::from(current_point) > scheduler_expiration_point)
    }

    fn get_min_fee_numerator(&self) -> anyhow::Result<u64> {
        self.get_base_fee_numerator_by_period(self.number_of_period.into())
    }

    fn get_max_fee_numerator(&self) -> anyhow::Result<u64> {
        Ok(self.cliff_fee_numerator)
    }
}
