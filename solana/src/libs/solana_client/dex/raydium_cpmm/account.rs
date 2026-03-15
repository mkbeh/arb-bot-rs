#![allow(clippy::identity_op)]

use bytemuck::{Pod, Zeroable};
use solana_sdk::pubkey::Pubkey;

use crate::libs::solana_client::{
    dex::raydium_cpmm::{
        constants::*,
        curve,
        curve::TradeDirection,
        error::ErrorCode,
        token_2022::{get_transfer_fee, get_transfer_inverse_fee},
    },
    metrics::*,
    pool::*,
    registry::DexEntity,
};

const CPMM_COMPUTE_UNITS: u32 = 41_250;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CreatorFeeOn {
    /// Both token0 and token1 can be used as trade fees.
    /// It depends on what the input token is.
    BothToken,
    /// Only token0 can be used as trade fees.
    OnlyToken0,
    /// Only token1 can be used as trade fees.
    OnlyToken1,
}

impl CreatorFeeOn {
    fn from_u8(value: u8) -> anyhow::Result<Self> {
        match value {
            0 => Ok(Self::BothToken),
            1 => Ok(Self::OnlyToken0),
            2 => Ok(Self::OnlyToken1),
            _ => Err(ErrorCode::InvalidFeeModel.into()),
        }
    }

    #[must_use]
    pub fn to_u8(&self) -> u8 {
        match self {
            Self::BothToken => 0u8,
            Self::OnlyToken0 => 1u8,
            Self::OnlyToken1 => 2u8,
        }
    }
}

/// Holds the current owner of the factory
#[repr(C, packed)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct AmmConfig {
    /// Bump to identify PDA
    pub bump: u8,
    /// Status to control if new pool can be create
    pub disable_create_pool: u8,
    /// Config index
    pub index: u16,
    /// The trade fee, denominated in hundredths of a bip (10^-6)
    pub trade_fee_rate: u64,
    /// The protocol fee
    pub protocol_fee_rate: u64,
    /// The fund fee, denominated in hundredths of a bip (10^-6)
    pub fund_fee_rate: u64,
    /// Fee for create a new pool
    pub create_pool_fee: u64,
    /// Address of the protocol fee owner
    pub protocol_owner: [u8; 32],
    /// Address of the fund fee owner
    pub fund_owner: [u8; 32],
    /// The pool creator fee, denominated in hundredths of a bip (10^-6)
    pub creator_fee_rate: u64,
    /// padding
    pub padding: [u64; 15],
}

impl DexEntity for AmmConfig {
    const PROGRAM_ID: Pubkey = RAYDIUM_CPMM_ID;
    const DISCRIMINATOR: &'static [u8] = &[218, 244, 33, 104, 203, 203, 43, 111];
    const DATA_SIZE: usize = 8 + 1 + 1 + 2 + 4 * 8 + 32 * 2 + 8 + 8 * 15; // 236

    fn deserialize(data: &[u8]) -> Option<Self> {
        Self::deserialize_bytemuck(data)
    }
}

#[repr(C, packed)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct PoolState {
    /// Which config the pool belongs
    pub amm_config: [u8; 32],
    /// pool creator
    pub pool_creator: [u8; 32],
    /// Token A
    pub token_0_vault: [u8; 32],
    /// Token B
    pub token_1_vault: [u8; 32],

    /// Pool tokens are issued when A or B tokens are deposited.
    /// Pool tokens can be withdrawn back to the original A or B token.
    pub lp_mint: [u8; 32],
    /// Mint information for token A
    pub token_0_mint: [u8; 32],
    /// Mint information for token B
    pub token_1_mint: [u8; 32],

    /// token_0 program
    pub token_0_program: [u8; 32],
    /// token_1 program
    pub token_1_program: [u8; 32],

    /// observation account to store oracle data
    pub observation_key: [u8; 32],

    pub auth_bump: u8,
    /// Bitwise representation of the state of the pool
    /// bit0, 1: disable deposit(value is 1), 0: normal
    /// bit1, 1: disable withdraw(value is 2), 0: normal
    /// bit2, 1: disable swap(value is 4), 0: normal
    pub status: u8,

    pub lp_mint_decimals: u8,
    /// mint0 and mint1 decimals
    pub mint_0_decimals: u8,
    pub mint_1_decimals: u8,

    /// True circulating supply without burns and lock ups
    pub lp_supply: u64,
    /// The amounts of token_0 and token_1 that are owed to the liquidity provider.
    pub protocol_fees_token_0: u64,
    pub protocol_fees_token_1: u64,

    pub fund_fees_token_0: u64,
    pub fund_fees_token_1: u64,

    /// The timestamp allowed for swap in the pool.
    pub open_time: u64,
    /// recent epoch
    pub recent_epoch: u64,

    /// Creator fee collect mode
    /// 0: both token_0 and token_1 can be used as trade fees. It depends on what the input token
    /// is when swapping 1: only token_0 as trade fee
    /// 2: only token_1 as trade fee
    pub creator_fee_on: u8,
    pub enable_creator_fee: u8,
    pub padding1: [u8; 6],
    pub creator_fees_token_0: u64,
    pub creator_fees_token_1: u64,
    /// padding for future updates
    pub padding: [u64; 28],
}

impl DexEntity for PoolState {
    const PROGRAM_ID: Pubkey = RAYDIUM_CPMM_ID;
    const DISCRIMINATOR: &'static [u8] = &[247, 237, 227, 245, 215, 195, 222, 70];
    const DATA_SIZE: usize = 8 + 10 * 32 + 1 * 5 + 8 * 7 + 1 * 2 + 6 * 1 + 2 * 8 + 8 * 28; // 637

    fn deserialize(data: &[u8]) -> Option<Self> {
        Self::deserialize_bytemuck(data)
    }
}

impl DexPool for PoolState {
    fn get_mint_a(&self) -> Pubkey {
        Pubkey::from(self.token_0_mint)
    }

    fn get_mint_b(&self) -> Pubkey {
        Pubkey::from(self.token_1_mint)
    }

    fn get_vault_pubkeys(&self) -> Option<(Pubkey, Pubkey)> {
        Some((
            Pubkey::from(self.token_0_vault),
            Pubkey::from(self.token_1_vault),
        ))
    }

    fn quote(&self, ctx: &QuoteContext) -> anyhow::Result<QuoteResult> {
        let Some(AmmConfigType::Cpmm(ref amm_config)) = ctx.amm_config else {
            anyhow::bail!("Missing AmmConfig for Raydium CPMM")
        };

        let (vault_0, vault_1) = ctx
            .vaults
            .ok_or_else(|| anyhow::anyhow!("Missing vault amounts for Raydium CPMM"))?;

        let (total_token_0_amount, total_token_1_amount) =
            self.vault_amount_without_fee(vault_0, vault_1)?;

        let zero_for_one = ctx.a_to_b;

        let (input_vault_amount, output_vault_amount) = if zero_for_one {
            (total_token_0_amount, total_token_1_amount)
        } else {
            (total_token_1_amount, total_token_0_amount)
        };

        let (mint_input, mint_output) = if zero_for_one {
            (ctx.unpack_pod_mint_in()?, ctx.unpack_pod_mint_out()?)
        } else {
            (ctx.unpack_pod_mint_out()?, ctx.unpack_pod_mint_in()?)
        };

        let creator_fee_rate = self.adjust_creator_fee_rate(amm_config.creator_fee_rate);
        let is_creator_fee_on_input = self.is_creator_fee_on_input(zero_for_one.into())?;

        match ctx.quote_type {
            QuoteType::ExactIn(amount) => {
                let transfer_fee_in = get_transfer_fee(&mint_input, ctx.clock.epoch, amount);
                let actual_amount_in = amount.saturating_sub(transfer_fee_in);

                let result = curve::CurveCalculator::swap_base_input(
                    u128::from(actual_amount_in),
                    u128::from(input_vault_amount),
                    u128::from(output_vault_amount),
                    amm_config.trade_fee_rate,
                    creator_fee_rate,
                    amm_config.protocol_fee_rate,
                    amm_config.fund_fee_rate,
                    is_creator_fee_on_input,
                )
                .ok_or_else(|| anyhow::anyhow!("swap_base_input returned None"))?;

                let amount_out = u64::try_from(result.output_amount)?;
                let transfer_fee = get_transfer_fee(&mint_output, ctx.clock.epoch, amount_out);
                let total_amount_out = amount_out
                    .checked_sub(transfer_fee)
                    .ok_or_else(|| anyhow::anyhow!("overflow in total_amount_out"))?;

                let total_fee = u64::try_from(
                    result.trade_fee + result.protocol_fee + result.fund_fee + result.creator_fee,
                )?;

                Ok(QuoteResult {
                    steps: vec![],
                    total_amount_in_gross: amount,
                    total_amount_in_net: actual_amount_in,
                    total_amount_out,
                    total_fee,
                    compute_units: CPMM_COMPUTE_UNITS,
                })
            }

            QuoteType::ExactOut(amount) => {
                let out_transfer_fee =
                    get_transfer_inverse_fee(&mint_output, ctx.clock.epoch, amount);
                let actual_amount_out = amount
                    .checked_add(out_transfer_fee)
                    .ok_or_else(|| anyhow::anyhow!("overflow in actual_amount_out"))?;

                let result = curve::CurveCalculator::swap_base_output(
                    u128::from(actual_amount_out),
                    u128::from(input_vault_amount),
                    u128::from(output_vault_amount),
                    amm_config.trade_fee_rate,
                    creator_fee_rate,
                    amm_config.protocol_fee_rate,
                    amm_config.fund_fee_rate,
                    is_creator_fee_on_input,
                )
                .ok_or_else(|| anyhow::anyhow!("swap_base_output returned None"))?;

                let source_amount_swapped = u64::try_from(result.input_amount)?;
                let amount_in_transfer_fee =
                    get_transfer_inverse_fee(&mint_input, ctx.clock.epoch, source_amount_swapped);

                let input_transfer_amount = source_amount_swapped
                    .checked_add(amount_in_transfer_fee)
                    .ok_or_else(|| anyhow::anyhow!("overflow in input_transfer_amount"))?;

                let total_fee = u64::try_from(
                    result.trade_fee + result.protocol_fee + result.fund_fee + result.creator_fee,
                )?;

                Ok(QuoteResult {
                    steps: vec![],
                    total_amount_in_gross: input_transfer_amount,
                    total_amount_in_net: source_amount_swapped,
                    total_amount_out: amount,
                    total_fee,
                    compute_units: CPMM_COMPUTE_UNITS,
                })
            }
        }
    }
}

impl DexMetrics for PoolState {
    fn dex_name(&self) -> &'static str {
        DEX_RAYDIUM_CPMM
    }
}

impl PoolState {
    pub fn vault_amount_without_fee(
        &self,
        vault_0: u64,
        vault_1: u64,
    ) -> anyhow::Result<(u64, u64)> {
        let fees_token_0 = self
            .protocol_fees_token_0
            .checked_add(self.fund_fees_token_0)
            .ok_or(ErrorCode::MathOverflow)?
            .checked_add(self.creator_fees_token_0)
            .ok_or(ErrorCode::MathOverflow)?;
        let fees_token_1 = self
            .protocol_fees_token_1
            .checked_add(self.fund_fees_token_1)
            .ok_or(ErrorCode::MathOverflow)?
            .checked_add(self.creator_fees_token_1)
            .ok_or(ErrorCode::MathOverflow)?;
        Ok((
            vault_0
                .checked_sub(fees_token_0)
                .ok_or(ErrorCode::InsufficientVault)?,
            vault_1
                .checked_sub(fees_token_1)
                .ok_or(ErrorCode::InsufficientVault)?,
        ))
    }

    #[must_use]
    pub fn adjust_creator_fee_rate(&self, creator_fee_rate: u64) -> u64 {
        if self.enable_creator_fee != 0 {
            creator_fee_rate
        } else {
            0
        }
    }

    // Determine the method used by the creator to calculate transaction fees
    pub fn is_creator_fee_on_input(&self, direction: TradeDirection) -> anyhow::Result<bool> {
        let fee_on = CreatorFeeOn::from_u8(self.creator_fee_on)?;
        Ok(matches!(
            (fee_on, direction),
            (CreatorFeeOn::BothToken, _)
                | (CreatorFeeOn::OnlyToken0, TradeDirection::ZeroForOne)
                | (CreatorFeeOn::OnlyToken1, TradeDirection::OneForZero)
        ))
    }
}
