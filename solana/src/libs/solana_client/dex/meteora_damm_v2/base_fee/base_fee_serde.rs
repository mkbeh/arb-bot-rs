use super::{
    BaseFeeHandler, fee_market_cap_scheduler::PodAlignedFeeMarketCapScheduler,
    fee_rate_limiter::PodAlignedFeeRateLimiter, fee_time_scheduler::PodAlignedFeeTimeScheduler,
};
use crate::libs::solana_client::dex::meteora_damm_v2::{
    BaseFeeInfo, error::PoolError, state::BaseFeeMode,
};

pub trait BaseFeeHandlerBuilder {
    fn get_base_fee_handler(&self) -> anyhow::Result<Box<dyn BaseFeeHandler>>;
}

pub trait BaseFeeEnumReader {
    const BASE_FEE_MODE_OFFSET: usize;
    fn get_base_fee_mode(&self) -> anyhow::Result<BaseFeeMode>;
}

impl BaseFeeHandlerBuilder for BaseFeeInfo {
    fn get_base_fee_handler(&self) -> anyhow::Result<Box<dyn BaseFeeHandler>> {
        let base_fee_mode = self.get_base_fee_mode()?;
        match base_fee_mode {
            BaseFeeMode::FeeTimeSchedulerExponential | BaseFeeMode::FeeTimeSchedulerLinear => {
                let fee_time_scheduler =
                    *bytemuck::try_from_bytes::<PodAlignedFeeTimeScheduler>(&self.data)
                        .map_err(|_| PoolError::UndeterminedError)?;
                Ok(Box::new(fee_time_scheduler))
            }
            BaseFeeMode::RateLimiter => {
                let fee_rate_limiter =
                    *bytemuck::try_from_bytes::<PodAlignedFeeRateLimiter>(&self.data)
                        .map_err(|_| PoolError::UndeterminedError)?;
                Ok(Box::new(fee_rate_limiter))
            }
            BaseFeeMode::FeeMarketCapSchedulerExponential
            | BaseFeeMode::FeeMarketCapSchedulerLinear => {
                let fee_market_cap_scheduler =
                    *bytemuck::try_from_bytes::<PodAlignedFeeMarketCapScheduler>(&self.data)
                        .map_err(|_| PoolError::UndeterminedError)?;
                Ok(Box::new(fee_market_cap_scheduler))
            }
        }
    }
}

impl BaseFeeEnumReader for BaseFeeInfo {
    const BASE_FEE_MODE_OFFSET: usize = 8;
    fn get_base_fee_mode(&self) -> anyhow::Result<BaseFeeMode> {
        let mode_byte = self
            .data
            .get(Self::BASE_FEE_MODE_OFFSET)
            .ok_or(PoolError::UndeterminedError)?;
        Ok(BaseFeeMode::try_from(*mode_byte).map_err(|_| PoolError::InvalidBaseFeeMode)?)
    }
}
