use bytemuck::{Pod, Zeroable};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use solana_sdk::pubkey::Pubkey;

use crate::libs::solana_client::{
    dex::meteora_damm_v2::{
        base_fee::BaseFeeHandlerBuilder,
        constants::*,
        error::PoolError,
        fee::*,
        liquidity_handler::*,
        math::{
            safe_math::{SafeCast, SafeMath},
            u128x128_math::Rounding,
            utils_math::*,
        },
        params::*,
        state::*,
        utils::{activation_handler::*, token::*},
    },
    metrics::*,
    pool::*,
    registry::DexEntity,
};

const DAMM_V2_COMPUTE_UNITS: u32 = 48_000;

/// collect fee mode
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, IntoPrimitive, TryFromPrimitive)]
pub enum CollectFeeMode {
    /// Both token, in this mode only out token is collected
    BothToken,
    /// Only token B, we just need token B, because if user want to collect fee in token A, they
    /// just need to flip order of tokens
    OnlyB,
    /// In the compounding, a percentage fees will be accumulated in liquidity, while remainings are
    /// used for clamining, fees are always be in token B Pool with compounding won't have price
    /// range, instead of using constant-product formula: x * y = constant
    Compounding,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, IntoPrimitive, TryFromPrimitive)]
pub enum LayoutVersion {
    V0, // 0
    V1, // 1
}

/// pool status
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, IntoPrimitive, TryFromPrimitive)]
pub enum PoolStatus {
    Enable,
    Disable,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct Pool {
    /// Pool fee
    pub pool_fees: PoolFeesStruct,
    /// token a mint
    pub token_a_mint: [u8; 32],
    /// token b mint
    pub token_b_mint: [u8; 32],
    /// token a vault
    pub token_a_vault: [u8; 32],
    /// token b vault
    pub token_b_vault: [u8; 32],
    /// Whitelisted vault to be able to buy pool before activation_point
    pub whitelisted_vault: [u8; 32],
    /// padding, previously partner pubkey, be careful when using this field
    pub padding_0: [u8; 32],
    /// liquidity share
    pub liquidity: [u64; 2],
    /// padding, previous reserve amount, be careful to use that field
    pub padding_1: [u64; 2],
    /// protocol a fee
    pub protocol_a_fee: u64,
    /// protocol b fee
    pub protocol_b_fee: u64,
    // padding for future use
    pub padding_2: [u64; 2],
    /// min price
    pub sqrt_min_price: [u64; 2],
    /// max price
    pub sqrt_max_price: [u64; 2],
    /// current price
    pub sqrt_price: [u64; 2],
    /// Activation point, can be slot or timestamp
    pub activation_point: u64,
    /// Activation type, 0 means by slot, 1 means by timestamp
    pub activation_type: u8,
    /// pool status, 0: enable, 1 disable
    pub pool_status: u8,
    /// token a flag
    pub token_a_flag: u8,
    /// token b flag
    pub token_b_flag: u8,
    /// 0 is collect fee in both token, 1 only collect fee only in token b
    pub collect_fee_mode: u8,
    /// pool type
    pub pool_type: u8,
    /// pool fee version, 0: max_fee is still capped at 50%, 1: max_fee is capped at 99%
    pub fee_version: u8,
    /// padding
    pub padding_3: u8,
    /// cumulative
    pub fee_a_per_liquidity: [u8; 32], // U256
    /// cumulative
    pub fee_b_per_liquidity: [u8; 32], // U256
    // permanent lock liquidity
    pub permanent_lock_liquidity: [u64; 2],
    /// metrics
    pub metrics: PoolMetrics,
    /// pool creator
    pub creator: [u8; 32],
    /// token a amount
    pub token_a_amount: u64,
    /// token b amount
    pub token_b_amount: u64,
    /// layout version: version 0: haven't track token_a_amount and token_b_amount, version 1:
    /// track token_a_amount and token_b_amount
    pub layout_version: u8,
    /// Padding for further use
    pub padding_4: [u8; 7],
    /// Padding for further use
    pub padding_5: [u64; 3],
    /// Farming reward information
    pub reward_infos: [RewardInfo; NUM_REWARDS],
}

impl DexEntity for Pool {
    const PROGRAM_ID: Pubkey = METEORA_DAMM_V2_ID;
    const DISCRIMINATOR: &'static [u8] = &[241, 154, 109, 4, 17, 177, 109, 188];
    const DATA_SIZE: usize = 1112;

    fn deserialize(data: &[u8]) -> Option<Self> {
        Self::deserialize_bytemuck(data)
    }
}

impl DexPool for Pool {
    fn get_mint_a(&self) -> Pubkey {
        Pubkey::from(self.token_a_mint)
    }

    fn get_mint_b(&self) -> Pubkey {
        Pubkey::from(self.token_b_mint)
    }

    fn quote(&self, ctx: &QuoteContext) -> anyhow::Result<QuoteResult> {
        let current_timestamp = ctx.clock.unix_timestamp as u64;
        let current_point =
            get_current_point(self.activation_type, ctx.clock.slot, current_timestamp)?;

        if !is_swap_enable(self, current_point)? {
            anyhow::bail!("Meteora DAMM v2 pool is disabled")
        }

        let mut pool = *self;
        pool.update_pre_swap(current_timestamp)?;

        let a_to_b = ctx.a_to_b;
        let trade_direction = TradeDirection::from(a_to_b);

        let collect_fee_mode = CollectFeeMode::try_from(pool.collect_fee_mode)
            .map_err(|_| PoolError::InvalidCollectFeeMode)?;
        let fee_mode = FeeMode::get_fee_mode(collect_fee_mode, trade_direction, false);

        let (mint_in, mint_out) = if a_to_b {
            (ctx.unpack_pod_mint_in()?, ctx.unpack_pod_mint_out()?)
        } else {
            (ctx.unpack_pod_mint_out()?, ctx.unpack_pod_mint_in()?)
        };

        match ctx.quote_type {
            QuoteType::ExactIn(amount) => {
                let transfer_fee_in = get_transfer_fee(&mint_in, ctx.clock.epoch, amount);
                let actual_amount_in = amount.saturating_sub(transfer_fee_in);

                let result = pool.get_swap_result_from_exact_input(
                    actual_amount_in,
                    &fee_mode,
                    trade_direction,
                    current_point,
                )?;

                let transfer_fee_out =
                    get_transfer_fee(&mint_out, ctx.clock.epoch, result.output_amount);
                let total_amount_out = result.output_amount.saturating_sub(transfer_fee_out);

                let total_fee = result
                    .claiming_fee
                    .checked_add(result.protocol_fee)
                    .and_then(|f| f.checked_add(result.compounding_fee))
                    .and_then(|f| f.checked_add(result.referral_fee))
                    .ok_or_else(|| anyhow::anyhow!("fee overflow"))?;

                Ok(QuoteResult {
                    steps: vec![],
                    total_amount_in_gross: amount,
                    total_amount_in_net: actual_amount_in,
                    total_amount_out,
                    total_fee,
                    compute_units: DAMM_V2_COMPUTE_UNITS,
                })
            }

            QuoteType::ExactOut(amount) => {
                let out_transfer_fee = get_transfer_inverse_fee(&mint_out, ctx.clock.epoch, amount);
                let actual_amount_out = amount
                    .checked_add(out_transfer_fee)
                    .ok_or_else(|| anyhow::anyhow!("overflow in actual_amount_out"))?;

                let result = pool.get_swap_result_from_exact_output(
                    actual_amount_out,
                    &fee_mode,
                    trade_direction,
                    current_point,
                )?;

                let transfer_fee_in = get_transfer_inverse_fee(
                    &mint_in,
                    ctx.clock.epoch,
                    result.included_fee_input_amount,
                );
                let total_amount_in_gross = result
                    .included_fee_input_amount
                    .checked_add(transfer_fee_in)
                    .ok_or_else(|| anyhow::anyhow!("overflow in total_amount_in_gross"))?;

                let total_fee = result
                    .claiming_fee
                    .checked_add(result.protocol_fee)
                    .and_then(|f| f.checked_add(result.compounding_fee))
                    .and_then(|f| f.checked_add(result.referral_fee))
                    .ok_or_else(|| anyhow::anyhow!("fee overflow"))?;

                Ok(QuoteResult {
                    steps: vec![],
                    total_amount_in_gross,
                    total_amount_in_net: result.included_fee_input_amount,
                    total_amount_out: amount,
                    total_fee,
                    compute_units: DAMM_V2_COMPUTE_UNITS,
                })
            }
        }
    }
}

impl DexMetrics for Pool {
    fn dex_name(&self) -> &'static str {
        DEX_METEORA_DAMM_V2
    }
}

impl Pool {
    #[must_use]
    pub fn sqrt_price(&self) -> u128 {
        u128::from(self.sqrt_price[0]) | (u128::from(self.sqrt_price[1]) << 64)
    }

    #[must_use]
    pub fn liquidity(&self) -> u128 {
        u128::from(self.liquidity[0]) | (u128::from(self.liquidity[1]) << 64)
    }

    #[must_use]
    pub fn sqrt_min_price(&self) -> u128 {
        u128::from(self.sqrt_min_price[0]) | (u128::from(self.sqrt_min_price[1]) << 64)
    }

    #[must_use]
    pub fn sqrt_max_price(&self) -> u128 {
        u128::from(self.sqrt_max_price[0]) | (u128::from(self.sqrt_max_price[1]) << 64)
    }

    pub fn get_swap_result_from_exact_output(
        &self,
        amount_out: u64,
        fee_mode: &FeeMode,
        trade_direction: TradeDirection,
        current_point: u64,
    ) -> anyhow::Result<SwapResult2> {
        let mut actual_protocol_fee = 0;
        let mut actual_compounding_fee = 0;
        let mut actual_claiming_fee = 0;
        let mut actual_referral_fee = 0;

        let liquidity_handler = self.get_liquidity_handler()?;

        let max_fee_numerator = get_max_fee_numerator(self.fee_version)?;

        let included_fee_amount_out = if fee_mode.fees_on_input {
            amount_out
        } else {
            let trade_fee_numerator = self
                .pool_fees
                .get_total_trading_fee_from_excluded_fee_amount(
                    current_point,
                    self.activation_point,
                    amount_out,
                    trade_direction,
                    max_fee_numerator,
                    self.sqrt_price(),
                )?;

            let (included_fee_amount_out, fee_amount) =
                PoolFeesStruct::get_included_fee_amount(trade_fee_numerator, amount_out)?;

            let SplitFees {
                compounding_fee,
                claiming_fee,
                protocol_fee,
                referral_fee,
            } = self
                .pool_fees
                .split_fees(fee_amount, fee_mode.has_referral)?;

            actual_protocol_fee = protocol_fee;
            actual_claiming_fee = claiming_fee;
            actual_compounding_fee = compounding_fee;
            actual_referral_fee = referral_fee;

            included_fee_amount_out
        };

        let SwapAmountFromOutput {
            input_amount,
            next_sqrt_price,
        } = match trade_direction {
            TradeDirection::AtoB => {
                liquidity_handler.calculate_a_to_b_from_amount_out(included_fee_amount_out)
            }
            TradeDirection::BtoA => {
                liquidity_handler.calculate_b_to_a_from_amount_out(included_fee_amount_out)
            }
        }?;

        let included_fee_input_amount = if fee_mode.fees_on_input {
            let trade_fee_numerator = self
                .pool_fees
                .get_total_trading_fee_from_excluded_fee_amount(
                    current_point,
                    self.activation_point,
                    input_amount,
                    trade_direction,
                    max_fee_numerator,
                    self.sqrt_price(),
                )?;

            let (included_fee_input_amount, fee_amount) =
                PoolFeesStruct::get_included_fee_amount(trade_fee_numerator, input_amount)?;

            let SplitFees {
                claiming_fee,
                compounding_fee,
                protocol_fee,
                referral_fee,
            } = self
                .pool_fees
                .split_fees(fee_amount, fee_mode.has_referral)?;

            actual_protocol_fee = protocol_fee;
            actual_claiming_fee = claiming_fee;
            actual_compounding_fee = compounding_fee;
            actual_referral_fee = referral_fee;

            included_fee_input_amount
        } else {
            input_amount
        };

        Ok(SwapResult2 {
            amount_left: 0,
            included_fee_input_amount,
            excluded_fee_input_amount: input_amount,
            output_amount: amount_out,
            next_sqrt_price,
            claiming_fee: actual_claiming_fee,
            compounding_fee: actual_compounding_fee,
            protocol_fee: actual_protocol_fee,
            referral_fee: actual_referral_fee,
        })
    }

    pub fn get_swap_result_from_partial_input(
        &self,
        amount_in: u64,
        fee_mode: &FeeMode,
        trade_direction: TradeDirection,
        current_point: u64,
    ) -> anyhow::Result<SwapResult2> {
        let mut actual_protocol_fee = 0;
        let mut actual_claiming_fee = 0;
        let mut actual_compounding_fee = 0;
        let mut actual_referral_fee = 0;

        let liquidity_handler = self.get_liquidity_handler()?;

        let max_fee_numerator = get_max_fee_numerator(self.fee_version)?;

        let trade_fee_numerator = self
            .pool_fees
            .get_total_trading_fee_from_included_fee_amount(
                current_point,
                self.activation_point,
                amount_in,
                trade_direction,
                max_fee_numerator,
                self.sqrt_price(),
            )?;

        let mut actual_amount_in = if fee_mode.fees_on_input {
            let FeeOnAmountResult {
                amount,
                claiming_fee,
                compounding_fee,
                protocol_fee,
                referral_fee,
            } = self.pool_fees.get_fee_on_amount(
                amount_in,
                trade_fee_numerator,
                fee_mode.has_referral,
            )?;

            actual_protocol_fee = protocol_fee;
            actual_claiming_fee = claiming_fee;
            actual_compounding_fee = compounding_fee;
            actual_referral_fee = referral_fee;

            amount
        } else {
            amount_in
        };

        let SwapAmountFromInput {
            amount_left,
            output_amount,
            next_sqrt_price,
        } = match trade_direction {
            TradeDirection::AtoB => {
                liquidity_handler.calculate_a_to_b_from_partial_amount_in(actual_amount_in)
            }
            TradeDirection::BtoA => {
                liquidity_handler.calculate_b_to_a_from_partial_amount_in(actual_amount_in)
            }
        }?;

        let included_fee_input_amount = if amount_left > 0 {
            actual_amount_in = actual_amount_in.safe_sub(amount_left)?;

            if fee_mode.fees_on_input {
                let trade_fee_numerator = self
                    .pool_fees
                    .get_total_trading_fee_from_excluded_fee_amount(
                        current_point,
                        self.activation_point,
                        actual_amount_in,
                        trade_direction,
                        max_fee_numerator,
                        self.sqrt_price(),
                    )?;

                let (included_fee_amount_in, fee_amount) =
                    PoolFeesStruct::get_included_fee_amount(trade_fee_numerator, actual_amount_in)?;

                let SplitFees {
                    claiming_fee,
                    compounding_fee,
                    protocol_fee,
                    referral_fee,
                } = self
                    .pool_fees
                    .split_fees(fee_amount, fee_mode.has_referral)?;

                actual_protocol_fee = protocol_fee;
                actual_claiming_fee = claiming_fee;
                actual_compounding_fee = compounding_fee;
                actual_referral_fee = referral_fee;

                included_fee_amount_in
            } else {
                actual_amount_in
            }
        } else {
            amount_in
        };

        let actual_amount_out = if fee_mode.fees_on_input {
            output_amount
        } else {
            let FeeOnAmountResult {
                amount,
                claiming_fee,
                compounding_fee,
                protocol_fee,
                referral_fee,
            } = self.pool_fees.get_fee_on_amount(
                output_amount,
                trade_fee_numerator,
                fee_mode.has_referral,
            )?;

            actual_protocol_fee = protocol_fee;
            actual_claiming_fee = claiming_fee;
            actual_compounding_fee = compounding_fee;
            actual_referral_fee = referral_fee;

            amount
        };

        Ok(SwapResult2 {
            included_fee_input_amount,
            excluded_fee_input_amount: actual_amount_in,
            amount_left,
            output_amount: actual_amount_out,
            next_sqrt_price,
            claiming_fee: actual_claiming_fee,
            compounding_fee: actual_compounding_fee,
            protocol_fee: actual_protocol_fee,
            referral_fee: actual_referral_fee,
        })
    }

    pub fn get_swap_result_from_exact_input(
        &self,
        amount_in: u64,
        fee_mode: &FeeMode,
        trade_direction: TradeDirection,
        current_point: u64,
    ) -> anyhow::Result<SwapResult2> {
        let mut actual_protocol_fee = 0;
        let mut actual_claiming_fee = 0;
        let mut actual_compounding_fee = 0;
        let mut actual_referral_fee = 0;

        let liquidity_handler = self.get_liquidity_handler()?;

        let max_fee_numerator = get_max_fee_numerator(self.fee_version)?;

        // We can compute the trade_fee_numerator here. Instead of separately for amount_in, and
        // amount_out. This is because FeeRateLimiter (fee rate scale based on amount) only
        // applied when fee_mode.fees_on_input (a.k.a TradeDirection::QuoteToBase +
        // CollectFeeMode::QuoteToken) For the rest of the time, the fee rate is not
        // dependent on amount.
        let trade_fee_numerator = self
            .pool_fees
            .get_total_trading_fee_from_included_fee_amount(
                current_point,
                self.activation_point,
                amount_in,
                trade_direction,
                max_fee_numerator,
                self.sqrt_price(),
            )?;

        let actual_amount_in = if fee_mode.fees_on_input {
            let FeeOnAmountResult {
                amount,
                claiming_fee,
                compounding_fee,
                protocol_fee,
                referral_fee,
            } = self.pool_fees.get_fee_on_amount(
                amount_in,
                trade_fee_numerator,
                fee_mode.has_referral,
            )?;

            actual_claiming_fee = claiming_fee;
            actual_compounding_fee = compounding_fee;
            actual_protocol_fee = protocol_fee;
            actual_referral_fee = referral_fee;

            amount
        } else {
            amount_in
        };

        let SwapAmountFromInput {
            output_amount,
            next_sqrt_price,
            amount_left,
        } = match trade_direction {
            TradeDirection::AtoB => {
                liquidity_handler.calculate_a_to_b_from_amount_in(actual_amount_in)
            }
            TradeDirection::BtoA => {
                liquidity_handler.calculate_b_to_a_from_amount_in(actual_amount_in)
            }
        }?;

        let actual_amount_out = if fee_mode.fees_on_input {
            output_amount
        } else {
            let FeeOnAmountResult {
                amount,
                claiming_fee,
                compounding_fee,
                protocol_fee,
                referral_fee,
            } = self.pool_fees.get_fee_on_amount(
                output_amount,
                trade_fee_numerator,
                fee_mode.has_referral,
            )?;

            actual_claiming_fee = claiming_fee;
            actual_compounding_fee = compounding_fee;
            actual_protocol_fee = protocol_fee;
            actual_referral_fee = referral_fee;

            amount
        };

        Ok(SwapResult2 {
            amount_left,
            included_fee_input_amount: amount_in,
            excluded_fee_input_amount: actual_amount_in,
            output_amount: actual_amount_out,
            next_sqrt_price,
            claiming_fee: actual_claiming_fee,
            compounding_fee: actual_compounding_fee,
            protocol_fee: actual_protocol_fee,
            referral_fee: actual_referral_fee,
        })
    }

    pub fn update_pre_swap(&mut self, current_timestamp: u64) -> anyhow::Result<()> {
        if self.pool_fees.dynamic_fee.is_dynamic_fee_enable() {
            self.pool_fees
                .dynamic_fee
                .update_references(self.sqrt_price(), current_timestamp)?;
        }
        Ok(())
    }

    pub fn get_liquidity_handler(&self) -> anyhow::Result<Box<dyn LiquidityHandler>> {
        let collect_fee_mode: CollectFeeMode = self.collect_fee_mode.safe_cast()?;
        if collect_fee_mode == CollectFeeMode::Compounding {
            Ok(Box::new(CompoundingLiquidity {
                token_a_amount: self.token_a_amount,
                token_b_amount: self.token_b_amount,
                liquidity: self.liquidity(),
            }))
        } else {
            Ok(Box::new(ConcentratedLiquidity {
                sqrt_max_price: self.sqrt_max_price(),
                sqrt_min_price: self.sqrt_min_price(),
                liquidity: self.liquidity(),
                sqrt_price: self.sqrt_price(),
            }))
        }
    }
}

/// Information regarding fee charges
/// trading_fee = amount * trade_fee_numerator / denominator
/// protocol_fee = trading_fee * protocol_fee_percentage / 100
/// referral_fee = protocol_fee * referral_percentage / 100
/// partner_fee = (protocol_fee - referral_fee) * partner_fee_percentage / denominator
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct PoolFeesStruct {
    /// Trade fees are extra token amounts that are held inside the token
    /// accounts during a trade, making the value of liquidity tokens rise.
    /// Trade fee numerator
    pub base_fee: BaseFeeStruct,

    /// Protocol trading fees are extra token amounts that are held inside the token
    /// accounts during a trade, with the equivalent in pool tokens minted to
    /// the protocol of the program.
    /// Protocol trade fee numerator
    pub protocol_fee_percent: u8,
    /// padding for future use
    pub padding_0: u8,
    /// referral fee
    pub referral_fee_percent: u8,
    /// padding
    pub padding_1: [u8; 3],
    /// compounding fee bps, only non-zero in CollectFeeMode::Compounding
    pub compounding_fee_bps: u16,

    /// dynamic fee
    pub dynamic_fee: DynamicFeeStruct,

    pub init_sqrt_price: [u64; 2],
}

impl PoolFeesStruct {
    #[must_use]
    pub fn init_sqrt_price(&self) -> u128 {
        u128::from(self.init_sqrt_price[0]) | (u128::from(self.init_sqrt_price[1]) << 64)
    }

    fn get_total_fee_numerator(
        &self,
        base_fee_numerator: u64,
        max_fee_numerator: u64,
    ) -> anyhow::Result<u64> {
        let dynamic_fee = self.dynamic_fee.get_variable_fee()?;
        let total_fee_numerator = dynamic_fee.safe_add(base_fee_numerator.into())?;
        let total_fee_numerator: u64 = total_fee_numerator
            .try_into()
            .map_err(|_| PoolError::TypeCastFailed)?;

        if total_fee_numerator > max_fee_numerator {
            Ok(max_fee_numerator)
        } else {
            Ok(total_fee_numerator)
        }
    }

    pub fn get_total_trading_fee_from_included_fee_amount(
        &self,
        current_point: u64,
        activation_point: u64,
        included_fee_amount: u64,
        trade_direction: TradeDirection,
        max_fee_numerator: u64,
        sqrt_price: u128,
    ) -> anyhow::Result<u64> {
        let base_fee_handler = self.base_fee.base_fee_info.get_base_fee_handler()?;

        let base_fee_numerator = base_fee_handler.get_base_fee_numerator_from_included_fee_amount(
            current_point,
            activation_point,
            trade_direction,
            included_fee_amount,
            self.init_sqrt_price(),
            sqrt_price,
        )?;

        self.get_total_fee_numerator(base_fee_numerator, max_fee_numerator)
    }

    pub fn get_total_trading_fee_from_excluded_fee_amount(
        &self,
        current_point: u64,
        activation_point: u64,
        excluded_fee_amount: u64,
        trade_direction: TradeDirection,
        max_fee_numerator: u64,
        sqrt_price: u128,
    ) -> anyhow::Result<u64> {
        let base_fee_handler = self.base_fee.base_fee_info.get_base_fee_handler()?;

        let base_fee_numerator = base_fee_handler.get_base_fee_numerator_from_excluded_fee_amount(
            current_point,
            activation_point,
            trade_direction,
            excluded_fee_amount,
            self.init_sqrt_price(),
            sqrt_price,
        )?;

        self.get_total_fee_numerator(base_fee_numerator, max_fee_numerator)
    }

    pub fn get_fee_on_amount(
        &self,
        amount: u64,
        trade_fee_numerator: u64,
        has_referral: bool,
    ) -> anyhow::Result<FeeOnAmountResult> {
        let (amount, trading_fee) = Self::get_excluded_fee_amount(trade_fee_numerator, amount)?;

        let SplitFees {
            claiming_fee,
            compounding_fee,
            protocol_fee,
            referral_fee,
        } = self.split_fees(trading_fee, has_referral)?;

        Ok(FeeOnAmountResult {
            amount,
            claiming_fee,
            compounding_fee,
            protocol_fee,
            referral_fee,
        })
    }

    pub fn get_excluded_fee_amount(
        trade_fee_numerator: u64,
        included_fee_amount: u64,
    ) -> anyhow::Result<(u64, u64)> {
        let trading_fee: u64 = safe_mul_div_cast_u64(
            included_fee_amount,
            trade_fee_numerator,
            FEE_DENOMINATOR,
            Rounding::Up,
        )?;
        let excluded_fee_amount = included_fee_amount.safe_sub(trading_fee)?;
        Ok((excluded_fee_amount, trading_fee))
    }

    pub fn get_included_fee_amount(
        trade_fee_numerator: u64,
        excluded_fee_amount: u64,
    ) -> anyhow::Result<(u64, u64)> {
        let included_fee_amount: u64 = safe_mul_div_cast_u64(
            excluded_fee_amount,
            FEE_DENOMINATOR,
            FEE_DENOMINATOR.safe_sub(trade_fee_numerator)?,
            Rounding::Up,
        )?;
        let fee_amount = included_fee_amount.safe_sub(excluded_fee_amount)?;
        Ok((included_fee_amount, fee_amount))
    }

    pub fn split_fees(&self, fee_amount: u64, has_referral: bool) -> anyhow::Result<SplitFees> {
        let protocol_fee = safe_mul_div_cast_u64(
            fee_amount,
            self.protocol_fee_percent.into(),
            100,
            Rounding::Down,
        )?;

        // update trading fee
        let trading_fee: u64 = fee_amount.safe_sub(protocol_fee)?;

        let (compounding_fee, claiming_fee) = if self.compounding_fee_bps > 0 {
            let compounding_fee: u64 = safe_mul_div_cast_u64(
                trading_fee,
                self.compounding_fee_bps.into(),
                MAX_BASIS_POINT.into(),
                Rounding::Down,
            )?;
            let claiming_fee = trading_fee.safe_sub(compounding_fee)?;
            (compounding_fee, claiming_fee)
        } else {
            (0, trading_fee)
        };

        let referral_fee = if has_referral {
            safe_mul_div_cast_u64(
                protocol_fee,
                self.referral_fee_percent.into(),
                100,
                Rounding::Down,
            )?
        } else {
            0
        };

        let protocol_fee = protocol_fee.safe_sub(referral_fee)?;

        Ok(SplitFees {
            claiming_fee,
            compounding_fee,
            protocol_fee,
            referral_fee,
        })
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct BaseFeeStruct {
    pub base_fee_info: BaseFeeInfo,
    pub padding_1: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct BaseFeeInfo {
    pub data: [u8; 32],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct DynamicFeeStruct {
    pub initialized: u8, // 0, ignore for dynamic fee
    pub _padding: [u8; 7],
    pub max_volatility_accumulator: u32,
    pub variable_fee_control: u32,
    pub bin_step: u16,
    pub filter_period: u16,
    pub decay_period: u16,
    pub reduction_factor: u16,
    pub last_update_timestamp: u64,
    pub bin_step_u128: [u64; 2],
    pub sqrt_price_reference: [u64; 2], // reference sqrt price
    pub volatility_accumulator: [u64; 2],
    pub volatility_reference: [u64; 2], // decayed volatility accumulator
}

impl DynamicFeeStruct {
    #[must_use]
    pub fn volatility_accumulator(&self) -> u128 {
        u128::from(self.volatility_accumulator[0])
            | (u128::from(self.volatility_accumulator[1]) << 64)
    }

    #[must_use]
    pub fn volatility_reference(&self) -> u128 {
        u128::from(self.volatility_reference[0]) | (u128::from(self.volatility_reference[1]) << 64)
    }

    #[must_use]
    pub fn bin_step_u128(&self) -> u128 {
        u128::from(self.bin_step_u128[0]) | (u128::from(self.bin_step_u128[1]) << 64)
    }

    #[must_use]
    pub fn sqrt_price_reference(&self) -> u128 {
        u128::from(self.sqrt_price_reference[0]) | (u128::from(self.sqrt_price_reference[1]) << 64)
    }

    #[must_use]
    pub fn is_dynamic_fee_enable(&self) -> bool {
        self.initialized != 0
    }

    pub fn set_sqrt_price_reference(&mut self, value: u128) {
        self.sqrt_price_reference[0] = value as u64;
        self.sqrt_price_reference[1] = (value >> 64) as u64;
    }

    pub fn set_volatility_reference(&mut self, value: u128) {
        self.volatility_reference[0] = value as u64;
        self.volatility_reference[1] = (value >> 64) as u64;
    }

    pub fn get_variable_fee(&self) -> anyhow::Result<u128> {
        if self.is_dynamic_fee_enable() {
            let square_vfa_bin: u128 = self
                .volatility_accumulator()
                .safe_mul(self.bin_step.into())?
                .checked_pow(2)
                .ok_or(PoolError::TypeCastFailed)?;
            // Variable fee control, volatility accumulator, bin step are in basis point unit
            // (10_000) This is 1e20. Which > 1e9. Scale down it to 1e9 unit and ceiling
            // the remaining.
            let v_fee = square_vfa_bin.safe_mul(self.variable_fee_control.into())?;

            let scaled_v_fee = v_fee.safe_add(99_999_999_999)?.safe_div(100_000_000_000)?;

            Ok(scaled_v_fee)
        } else {
            Ok(0)
        }
    }

    pub fn update_references(
        &mut self,
        sqrt_price_current: u128,
        current_timestamp: u64,
    ) -> anyhow::Result<()> {
        // it is fine to use saturating_sub, because never a chance current_timestamp is lesser than
        // last_update_timestamp on-chain but that can benefit off-chain components for
        // simulation when clock is not synced and pool is high frequency trading
        // furthermore, the function doesn't update fee in pre-swap, so quoting won't be affected
        let elapsed = current_timestamp.saturating_sub(self.last_update_timestamp);
        // Not high frequency trade
        if elapsed >= self.filter_period as u64 {
            // Update sqrt of last transaction
            self.set_sqrt_price_reference(sqrt_price_current);
            // filter period < t < decay_period. Decay time window.
            if elapsed < self.decay_period as u64 {
                let volatility_reference = self
                    .volatility_accumulator()
                    .safe_mul(self.reduction_factor.into())?
                    .safe_div(MAX_BASIS_POINT.into())?;

                self.set_volatility_reference(volatility_reference);
            }
            // Out of decay time window
            else {
                self.set_volatility_reference(0);
            }
        }
        Ok(())
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct PoolMetrics {
    pub total_lp_a_fee: [u64; 2],
    pub total_lp_b_fee: [u64; 2],
    pub total_protocol_a_fee: u64,
    pub total_protocol_b_fee: u64,
    pub padding_0: [u64; 2],
    pub total_position: u64,
    pub padding: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct RewardInfo {
    /// Indicates if the reward has been initialized
    pub initialized: u8,
    /// reward token flag
    pub reward_token_flag: u8,
    /// padding
    pub _padding_0: [u8; 6],
    /// Padding to ensure `reward_rate: u128` is 16-byte aligned
    pub _padding_1: [u8; 8], // 8 bytes
    /// Reward token mint.
    pub mint: [u8; 32],
    /// Reward vault token account.
    pub vault: [u8; 32],
    /// Authority account that allows to fund rewards
    pub funder: [u8; 32],
    /// reward duration
    pub reward_duration: u64,
    /// reward duration end
    pub reward_duration_end: u64,
    /// reward rate
    pub reward_rate: u128,
    /// Reward per token stored
    pub reward_per_token_stored: [u8; 32], // U256
    /// The last time reward states were updated.
    pub last_update_time: u64,
    /// Accumulated seconds when the farm distributed rewards but the bin was empty.
    /// These rewards will be carried over to the next reward time window.
    pub cumulative_seconds_with_empty_liquidity_reward: u64,
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub struct SwapResult2 {
    // This is excluded_transfer_fee_amount_in
    pub included_fee_input_amount: u64,
    pub excluded_fee_input_amount: u64,
    pub amount_left: u64,
    pub output_amount: u64,
    pub next_sqrt_price: u128,
    pub claiming_fee: u64,
    pub protocol_fee: u64,
    pub compounding_fee: u64, // previous is partner_fee, now will be reused for compounding_fee
    pub referral_fee: u64,
}

pub struct SwapAmountFromInput {
    pub output_amount: u64,
    pub next_sqrt_price: u128,
    pub amount_left: u64,
}

pub struct SwapAmountFromOutput {
    pub input_amount: u64,
    pub next_sqrt_price: u128,
}
