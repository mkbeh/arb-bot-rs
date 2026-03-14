//! Swap calculations

use super::{constant_product::ConstantProductCurve, fees::Fees};

/// The direction of a trade, since curves can be specialized to treat each
/// token differently (by adding offsets or weights)
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TradeDirection {
    /// Input token 0, output token 1
    ZeroForOne,
    /// Input token 1, output token 0
    OneForZero,
}

impl From<bool> for TradeDirection {
    fn from(zero_for_one: bool) -> Self {
        if zero_for_one {
            Self::ZeroForOne
        } else {
            Self::OneForZero
        }
    }
}

/// Encodes all results of swapping from a source token to a destination token
#[derive(Debug, PartialEq)]
pub struct SwapResult {
    /// The new amount in the input token vault, excluding  trade fees
    pub new_input_vault_amount: u128,
    /// The new amount in the output token vault, excluding trade fees
    pub new_output_vault_amount: u128,
    /// User's input amount, including trade fees, excluding transfer fees
    pub input_amount: u128,
    /// The amount to be transfer to user, including transfer fees
    pub output_amount: u128,
    /// Amount of input tokens going to pool holders
    pub trade_fee: u128,
    /// Amount of input tokens going to protocol
    pub protocol_fee: u128,
    /// Amount of input tokens going to protocol team
    pub fund_fee: u128,
    /// Amount of fee tokens going to creator
    pub creator_fee: u128,
}

/// Concrete struct to wrap around the trait object which performs calculation.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CurveCalculator {}

impl CurveCalculator {
    /// Subtract fees and calculate how much destination token will be provided
    /// given an amount of source token.
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn swap_base_input(
        input_amount: u128,
        input_vault_amount: u128,
        output_vault_amount: u128,
        trade_fee_rate: u64,
        creator_fee_rate: u64,
        protocol_fee_rate: u64,
        fund_fee_rate: u64,
        is_creator_fee_on_input: bool,
    ) -> Option<SwapResult> {
        let mut creator_fee = 0;
        let trade_fee: u128;

        let input_amount_less_fees = if is_creator_fee_on_input {
            let total_fee = Fees::trading_fee(input_amount, trade_fee_rate + creator_fee_rate)?;
            creator_fee = Fees::split_creator_fee(total_fee, trade_fee_rate, creator_fee_rate)?;
            trade_fee = total_fee - creator_fee;
            input_amount.checked_sub(total_fee)?
        } else {
            trade_fee = Fees::trading_fee(input_amount, trade_fee_rate)?;
            input_amount.checked_sub(trade_fee)?
        };
        let protocol_fee = Fees::protocol_fee(trade_fee, protocol_fee_rate)?;
        let fund_fee = Fees::fund_fee(trade_fee, fund_fee_rate)?;

        let output_amount_swapped = ConstantProductCurve::swap_base_input_without_fees(
            input_amount_less_fees,
            input_vault_amount,
            output_vault_amount,
        );

        let output_amount = if is_creator_fee_on_input {
            output_amount_swapped
        } else {
            creator_fee = Fees::creator_fee(output_amount_swapped, creator_fee_rate)?;
            output_amount_swapped.checked_sub(creator_fee)?
        };

        Some(SwapResult {
            new_input_vault_amount: input_vault_amount.checked_add(input_amount_less_fees)?,
            new_output_vault_amount: output_vault_amount.checked_sub(output_amount_swapped)?,
            input_amount,
            output_amount,
            trade_fee,
            protocol_fee,
            fund_fee,
            creator_fee,
        })
    }

    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn swap_base_output(
        output_amount: u128,
        input_vault_amount: u128,
        output_vault_amount: u128,
        trade_fee_rate: u64,
        creator_fee_rate: u64,
        protocol_fee_rate: u64,
        fund_fee_rate: u64,
        is_creator_fee_on_input: bool,
    ) -> Option<SwapResult> {
        let trade_fee: u128;
        let mut creator_fee = 0;

        let actual_output_amount = if is_creator_fee_on_input {
            output_amount
        } else {
            let out_amount_with_creator_fee =
                Fees::calculate_pre_fee_amount(output_amount, creator_fee_rate)?;
            creator_fee = out_amount_with_creator_fee - output_amount;
            out_amount_with_creator_fee
        };

        let input_amount_swapped = ConstantProductCurve::swap_base_output_without_fees(
            actual_output_amount,
            input_vault_amount,
            output_vault_amount,
        );

        let input_amount = if is_creator_fee_on_input {
            let input_amount_with_fee = Fees::calculate_pre_fee_amount(
                input_amount_swapped,
                trade_fee_rate + creator_fee_rate,
            )
            .unwrap();
            let total_fee = input_amount_with_fee - input_amount_swapped;
            creator_fee = Fees::split_creator_fee(total_fee, trade_fee_rate, creator_fee_rate)?;
            trade_fee = total_fee - creator_fee;
            input_amount_with_fee
        } else {
            let input_amount_with_fee =
                Fees::calculate_pre_fee_amount(input_amount_swapped, trade_fee_rate).unwrap();
            trade_fee = input_amount_with_fee - input_amount_swapped;
            input_amount_with_fee
        };
        let protocol_fee = Fees::protocol_fee(trade_fee, protocol_fee_rate)?;
        let fund_fee = Fees::fund_fee(trade_fee, fund_fee_rate)?;
        Some(SwapResult {
            new_input_vault_amount: input_vault_amount.checked_add(input_amount_swapped)?,
            new_output_vault_amount: output_vault_amount.checked_sub(actual_output_amount)?,
            input_amount,
            output_amount,
            trade_fee,
            protocol_fee,
            fund_fee,
            creator_fee,
        })
    }
}
