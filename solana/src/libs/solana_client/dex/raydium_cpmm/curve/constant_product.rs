//! The Uniswap invariantConstantProductCurve::

#![allow(clippy::let_and_return)]

use crate::libs::solana_client::dex::raydium_cpmm::utils::CheckedCeilDiv;

/// ConstantProductCurve struct implementing CurveCalculator
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ConstantProductCurve;

impl ConstantProductCurve {
    /// Constant product swap ensures x * y = constant
    /// The constant product swap calculation, factored out of its class for reuse.
    ///
    /// This is guaranteed to work for all values such that:
    ///  - 1 <= source_vault_amount * destination_vault_amount <= u128::MAX
    ///  - 1 <= source_amount <= u64::MAX
    #[must_use]
    pub fn swap_base_input_without_fees(
        input_amount: u128,
        input_vault_amount: u128,
        output_vault_amount: u128,
    ) -> u128 {
        // (x + delta_x) * (y - delta_y) = x * y
        // delta_y = (delta_x * y) / (x + delta_x)
        let numerator = input_amount.checked_mul(output_vault_amount).unwrap();
        let denominator = input_vault_amount.checked_add(input_amount).unwrap();
        let output_amount = numerator.checked_div(denominator).unwrap();
        output_amount
    }

    #[must_use]
    pub fn swap_base_output_without_fees(
        output_amount: u128,
        input_vault_amount: u128,
        output_vault_amount: u128,
    ) -> u128 {
        // (x + delta_x) * (y - delta_y) = x * y
        // delta_x = (x * delta_y) / (y - delta_y)
        let numerator = input_vault_amount.checked_mul(output_amount).unwrap();
        let denominator = output_vault_amount.checked_sub(output_amount).unwrap();
        let input_amount = numerator.checked_ceil_div(denominator).unwrap();
        input_amount
    }
}
