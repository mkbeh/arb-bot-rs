use solana_sdk::{account::Account, clock::Clock};
use spl_token_2022::extension::StateWithExtensions;

use crate::libs::solana_client::pool::{AmmConfigType, LiquidityBitmap, LiquidityMap};

/// Specifies the type of swap simulation to perform.
pub enum QuoteType {
    /// Simulate a swap with an exact input amount.
    /// Returns the maximum output amount achievable for the given input.
    ExactIn(u64),

    /// Simulate a swap targeting an exact output amount.
    /// Returns the minimum input amount required to receive the desired output.
    ExactOut(u64),
}

/// Input parameters for a swap simulation.
pub struct QuoteContext<'a> {
    /// The type and amount of the swap (exact in or exact out).
    pub quote_type: QuoteType,

    /// Swap direction: `true` means token A → token B, `false` means B → A.
    pub a_to_b: bool,

    /// Current Solana clock (used for fee calculations and activation checks).
    pub clock: &'a Clock,

    /// Mint account of the input token (used for Token-2022 transfer fee calculation).
    pub mint_in: &'a Account,

    /// Mint account of the output token (used for Token-2022 transfer fee calculation).
    pub mint_out: &'a Account,

    /// Vault token amounts (amount_a, amount_b) for pools that use external vaults (CPMM, AMM).
    pub vaults: Option<(u64, u64)>,

    /// Protocol-specific liquidity arrays from cache.
    pub liquidity: Option<LiquidityMap<'a>>,

    /// Optional protocol-specific bitmap extension for locating liquidity arrays.
    pub bitmap: Option<LiquidityBitmap<'a>>,

    /// AMM config for the pool being quoted (contains fee rates and tick spacing).
    pub amm_config: Option<AmmConfigType>,
}

impl<'a> QuoteContext<'a> {
    pub fn mint_in_state(
        &self,
    ) -> anyhow::Result<StateWithExtensions<'_, spl_token_2022::state::Mint>> {
        Ok(StateWithExtensions::<spl_token_2022::state::Mint>::unpack(
            self.mint_in.data.as_ref(),
        )?)
    }

    pub fn mint_out_state(
        &self,
    ) -> anyhow::Result<StateWithExtensions<'_, spl_token_2022::state::Mint>> {
        Ok(StateWithExtensions::<spl_token_2022::state::Mint>::unpack(
            self.mint_out.data.as_ref(),
        )?)
    }
}

/// Result of a swap simulation.
pub struct QuoteResult {
    /// Step-by-step breakdown of each bin/tick crossed during the swap.
    pub steps: Vec<QuoteSwapResult>,

    /// Gross input amount deducted from the wallet, including Token-2022 transfer fee.
    /// Use this for flash loan sizing and wallet balance checks.
    pub total_amount_in_gross: u64,

    /// Net input amount that entered the pool after Token-2022 transfer fee deduction.
    /// Use this for flash loan repayment math.
    pub total_amount_in_net: u64,

    /// Final output amount received in the destination wallet after all fees.
    pub total_amount_out: u64,

    /// Total protocol and LP fees paid during the swap (already deducted from `total_amount_out`).
    pub total_fee: u64,

    /// Estimated Solana compute units required for the on-chain swap transaction.
    pub compute_units: u32,
}

/// Result of a single bin or tick swap step within a quote simulation.
pub struct QuoteSwapResult {
    /// The bin ID (Meteora DLMM) or tick index (CLMM) where this swap step occurred.
    pub pool_state_id: i32,

    /// Input amount consumed in this step, including protocol fees.
    pub amount_in: u64,

    /// Output amount produced in this step.
    pub amount_out: u64,

    /// Protocol and LP fee paid in this step.
    pub fee: u64,

    /// Bin or tick price at the time of this swap step (Q64.64 fixed-point).
    pub price: u128,
}
