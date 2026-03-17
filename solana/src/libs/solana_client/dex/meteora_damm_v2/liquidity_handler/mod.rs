pub mod compounding_liquidity;
pub use compounding_liquidity::*;

pub mod concentrated_liquidity;
pub use concentrated_liquidity::*;

#[cfg(test)]
use crate::libs::solana_client::dex::meteora_damm_v2::params::TradeDirection;
use crate::libs::solana_client::dex::meteora_damm_v2::{
    SwapAmountFromInput, SwapAmountFromOutput, math::u128x128_math::Rounding,
};

pub trait LiquidityHandler {
    fn get_amounts_for_modify_liquidity(
        &self,
        liquidity_delta: u128,
        round: Rounding,
    ) -> anyhow::Result<(u64, u64)>;

    fn calculate_a_to_b_from_amount_in(
        &self,
        amount_in: u64,
    ) -> anyhow::Result<SwapAmountFromInput>;

    fn calculate_b_to_a_from_amount_in(
        &self,
        amount_in: u64,
    ) -> anyhow::Result<SwapAmountFromInput>;

    fn calculate_a_to_b_from_partial_amount_in(
        &self,
        amount_in: u64,
    ) -> anyhow::Result<SwapAmountFromInput>;

    fn calculate_b_to_a_from_partial_amount_in(
        &self,
        amount_in: u64,
    ) -> anyhow::Result<SwapAmountFromInput>;

    fn calculate_a_to_b_from_amount_out(
        &self,
        amount_out: u64,
    ) -> anyhow::Result<SwapAmountFromOutput>;

    fn calculate_b_to_a_from_amount_out(
        &self,
        amount_out: u64,
    ) -> anyhow::Result<SwapAmountFromOutput>;

    fn get_reserves_amount(&self) -> anyhow::Result<(u64, u64)>;

    // Note: Due to different way of concentrated liquidity and compounding liquidity calculating
    // price, compounding and concentrated pools can update dynamic-fee volatility differently for
    // equivalent swap price moves. Additionally the market cap based base fee will also behave
    // differently: Concentrated Amount_In B to A -> Rounding Down
    // Concentrated Amount_Out B to A -> Rounding Up
    // Compounding Amount_In B to A -> Rounding Down
    // Compounding Amount_Out B to A -> Rounding Down
    fn get_next_sqrt_price(&self, next_sqrt_price: u128) -> anyhow::Result<u128>;

    #[cfg(test)]
    fn get_max_amount_in(&self, trade_direction: TradeDirection) -> anyhow::Result<u64>;
}
