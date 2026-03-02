use anyhow::{Context, Result, ensure};

use crate::libs::solana_client::dex::meteora_dlmm::{account::*, constants::*, types::*};

pub trait LbPairExtension {
    fn status(&self) -> Result<PairStatus>;
    fn compute_fee(&self, amount: u64) -> Result<u64>;
    fn get_total_fee(&self) -> Result<u128>;
    fn compute_fee_from_amount(&self, amount_with_fees: u64) -> Result<u64>;

    fn update_references(&mut self, current_timestamp: i64) -> Result<()>;
    fn update_volatility_accumulator(&mut self) -> Result<()>;
    fn advance_active_bin(&mut self, swap_for_y: bool) -> Result<()>;
}

impl LbPairExtension for LbPair {
    fn status(&self) -> Result<PairStatus> {
        Ok(self.status.try_into()?)
    }

    fn compute_fee(&self, amount: u64) -> Result<u64> {
        let total_fee_rate = self.get_total_fee()?;
        let denominator = u128::from(FEE_PRECISION)
            .checked_sub(total_fee_rate)
            .context("overflow")?;

        // Ceil division
        let fee = u128::from(amount)
            .checked_mul(total_fee_rate)
            .context("overflow")?
            .checked_add(denominator)
            .context("overflow")?
            .checked_sub(1)
            .context("overflow")?;

        let scaled_down_fee = fee.checked_div(denominator).context("overflow")?;

        Ok(scaled_down_fee.try_into().context("overflow")?)
    }

    fn get_total_fee(&self) -> Result<u128> {
        let total_fee_rate = self
            .get_base_fee()?
            .checked_add(self.get_variable_fee()?)
            .context("overflow")?;
        let total_fee_rate_cap = std::cmp::min(total_fee_rate, MAX_FEE_RATE.into());
        Ok(total_fee_rate_cap)
    }

    fn compute_fee_from_amount(&self, amount_with_fees: u64) -> Result<u64> {
        let total_fee_rate = self.get_total_fee()?;

        let fee_amount = u128::from(amount_with_fees)
            .checked_mul(total_fee_rate)
            .context("overflow")?
            .checked_add((FEE_PRECISION - 1).into())
            .context("overflow")?;

        let scaled_down_fee = fee_amount
            .checked_div(FEE_PRECISION.into())
            .context("overflow")?;

        Ok(scaled_down_fee.try_into().context("overflow")?)
    }

    fn update_references(&mut self, current_timestamp: i64) -> Result<()> {
        let v_params = &mut self.v_parameters;
        let s_params = &self.parameters;

        let elapsed = current_timestamp
            .checked_sub(v_params.last_update_timestamp)
            .context("overflow")?;

        // Not high frequency trade
        if elapsed >= s_params.filter_period as i64 {
            // Update active id of last transaction
            v_params.index_reference = self.active_id;
            // filter period < t < decay_period. Decay time window.
            if elapsed < s_params.decay_period as i64 {
                let volatility_reference = v_params
                    .volatility_accumulator
                    .checked_mul(s_params.reduction_factor as u32)
                    .context("overflow")?
                    .checked_div(BASIS_POINT_MAX as u32)
                    .context("overflow")?;

                v_params.volatility_reference = volatility_reference;
            }
            // Out of decay time window
            else {
                v_params.volatility_reference = 0;
            }
        }

        Ok(())
    }

    fn update_volatility_accumulator(&mut self) -> Result<()> {
        let v_params = &mut self.v_parameters;
        let s_params = &self.parameters;

        let delta_id = i64::from(v_params.index_reference)
            .checked_sub(self.active_id.into())
            .context("overflow")?
            .unsigned_abs();

        let volatility_accumulator = u64::from(v_params.volatility_reference)
            .checked_add(
                delta_id
                    .checked_mul(BASIS_POINT_MAX as u64)
                    .context("overflow")?,
            )
            .context("overflow")?;

        v_params.volatility_accumulator = std::cmp::min(
            volatility_accumulator,
            s_params.max_volatility_accumulator.into(),
        )
        .try_into()
        .context("overflow")?;

        Ok(())
    }

    fn advance_active_bin(&mut self, swap_for_y: bool) -> Result<()> {
        let next_active_bin_id = if swap_for_y {
            self.active_id.checked_sub(1)
        } else {
            self.active_id.checked_add(1)
        }
        .context("overflow")?;

        ensure!(
            next_active_bin_id >= MIN_BIN_ID && next_active_bin_id <= MAX_BIN_ID,
            "Insufficient liquidity"
        );

        self.active_id = next_active_bin_id;

        Ok(())
    }
}
