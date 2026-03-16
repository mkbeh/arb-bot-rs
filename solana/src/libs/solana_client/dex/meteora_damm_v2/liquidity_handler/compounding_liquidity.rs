use ruint::aliases::U256;

use super::LiquidityHandler;
#[cfg(test)]
use crate::libs::solana_client::dex::meteora_damm_v2::params::TradeDirection;
use crate::libs::solana_client::dex::meteora_damm_v2::{
    SwapAmountFromInput, SwapAmountFromOutput,
    error::PoolError,
    math::{
        safe_math::{SafeCast, SafeMath},
        u128x128_math::Rounding,
        utils_math::*,
    },
};

pub struct CompoundingLiquidity {
    pub token_a_amount: u64, // current token a reserve
    pub token_b_amount: u64, // current token_b_reserve
    pub liquidity: u128,     // current liquidity
}

impl LiquidityHandler for CompoundingLiquidity {
    fn get_amounts_for_modify_liquidity(
        &self,
        liquidity_delta: u128,
        round: Rounding,
    ) -> anyhow::Result<(u64, u64)> {
        let token_a_amount = safe_mul_div_cast_u128(
            liquidity_delta,
            self.token_a_amount.into(),
            self.liquidity,
            round,
        )?;
        let token_b_amount = safe_mul_div_cast_u128(
            liquidity_delta,
            self.token_b_amount.into(),
            self.liquidity,
            round,
        )?;

        Ok((token_a_amount.safe_cast()?, token_b_amount.safe_cast()?))
    }

    fn calculate_a_to_b_from_amount_in(
        &self,
        amount_in: u64,
    ) -> anyhow::Result<SwapAmountFromInput> {
        // a * b = (a + amount_in) * (b - output_amount)
        // => output_amount = b - a * b / (a + amount_in) = b * amount_in / (a + amount_in)
        let output_amount = safe_mul_div_cast_u64(
            self.token_b_amount,
            amount_in,
            self.token_a_amount.safe_add(amount_in)?,
            Rounding::Down,
        )?;

        Ok(SwapAmountFromInput {
            amount_left: 0,
            output_amount,
            next_sqrt_price: 0,
        })
    }

    fn calculate_b_to_a_from_amount_in(
        &self,
        amount_in: u64,
    ) -> anyhow::Result<SwapAmountFromInput> {
        // a * b = (b + amount_in) * (a - output_amount)
        // => output_amount = a - a * b / (b + amount_in) = a * amount_in / (b + amount_in)
        let output_amount = safe_mul_div_cast_u64(
            self.token_a_amount,
            amount_in,
            self.token_b_amount.safe_add(amount_in)?,
            Rounding::Down,
        )?;

        Ok(SwapAmountFromInput {
            amount_left: 0,
            output_amount,
            next_sqrt_price: 0, // dont need to care for next sqrt price now
        })
    }

    fn calculate_a_to_b_from_partial_amount_in(
        &self,
        amount_in: u64,
    ) -> anyhow::Result<SwapAmountFromInput> {
        // it is constant-product, so no price range
        self.calculate_a_to_b_from_amount_in(amount_in)
    }

    fn calculate_b_to_a_from_partial_amount_in(
        &self,
        amount_in: u64,
    ) -> anyhow::Result<SwapAmountFromInput> {
        // it is constant-product, so no price range
        self.calculate_b_to_a_from_amount_in(amount_in)
    }

    fn calculate_a_to_b_from_amount_out(
        &self,
        amount_out: u64,
    ) -> anyhow::Result<SwapAmountFromOutput> {
        // a * b = (a + amount_in) * (b - amount_out)
        // => amount_in = a * b / (b - amount_out) - a = a * amount_out / (b - amount_out)
        let input_amount = safe_mul_div_cast_u64(
            self.token_a_amount,
            amount_out,
            self.token_b_amount.safe_sub(amount_out)?,
            Rounding::Up,
        )?;
        Ok(SwapAmountFromOutput {
            input_amount,
            next_sqrt_price: 0, // dont need to care for next sqrt price now
        })
    }

    fn calculate_b_to_a_from_amount_out(
        &self,
        amount_out: u64,
    ) -> anyhow::Result<SwapAmountFromOutput> {
        // a * b = (b + amount_in) * (a - amount_out)
        // => amount_in = a * b / (a - amount_out) - b = b * amount_out / (a - amount_out)
        let input_amount = safe_mul_div_cast_u64(
            self.token_b_amount,
            amount_out,
            self.token_a_amount.safe_sub(amount_out)?,
            Rounding::Up,
        )?;
        Ok(SwapAmountFromOutput {
            input_amount,
            next_sqrt_price: 0, // dont need to care for next sqrt price now
        })
    }

    fn get_reserves_amount(&self) -> anyhow::Result<(u64, u64)> {
        Ok((self.token_a_amount, self.token_b_amount))
    }

    // xyk, the price is determined by the ratio of reserves and it always rounded down.
    fn get_next_sqrt_price(&self, _next_sqrt_price: u128) -> anyhow::Result<u128> {
        get_sqrt_price_from_amounts(self.token_a_amount, self.token_b_amount)
    }

    #[cfg(test)]
    fn get_max_amount_in(&self, _trade_direction: TradeDirection) -> anyhow::Result<u64> {
        Ok(u64::MAX)
    }
}

fn get_sqrt_price_from_amounts(token_a_amount: u64, token_b_amount: u64) -> anyhow::Result<u128> {
    let token_b_amount = U256::from(token_b_amount).safe_shl(128)?;
    let price = token_b_amount.safe_div(U256::from(token_a_amount))?;
    let sqrt_price = sqrt_u256(price).ok_or(PoolError::MathOverflow)?;
    Ok(sqrt_price
        .try_into()
        .map_err(|_| PoolError::TypeCastFailed)?)
}
