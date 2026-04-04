use bytemuck::{Pod, Zeroable};
use solana_sdk::pubkey::Pubkey;

use crate::libs::solana_client::{
    dex::raydium_amm::{constants::*, program::*},
    metrics::*,
    pool::*,
    registry::ProtocolEntity,
};

pub const AMM_COMPUTE_UNITS: u32 = 27_000;

#[repr(C, packed)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct AmmInfo {
    /// Initialized status.
    pub status: u64,
    /// Nonce used in program address.
    /// The program address is created deterministically with the nonce,
    /// amm program id, and amm account pubkey.  This program address has
    /// authority over the amm's token coin account, token pc account, and pool
    /// token mint.
    pub nonce: u64,
    /// max order count
    pub order_num: u64,
    /// within this range, 5 => 5% range
    pub depth: u64,
    /// coin decimal
    pub coin_decimals: u64,
    /// pc decimal
    pub pc_decimals: u64,
    /// amm machine state
    pub state: u64,
    /// amm reset_flag
    pub reset_flag: u64,
    /// min size 1->0.000001
    pub min_size: u64,
    /// vol_max_cut_ratio numerator, sys_decimal_value as denominator
    pub vol_max_cut_ratio: u64,
    /// amount wave numerator, sys_decimal_value as denominator
    pub amount_wave: u64,
    /// coinLotSize 1 -> 0.000001
    pub coin_lot_size: u64,
    /// pcLotSize 1 -> 0.000001
    pub pc_lot_size: u64,
    /// min_cur_price: (2 * amm.order_num * amm.pc_lot_size) * max_price_multiplier
    pub min_price_multiplier: u64,
    /// max_cur_price: (2 * amm.order_num * amm.pc_lot_size) * max_price_multiplier
    pub max_price_multiplier: u64,
    /// system decimal value, used to normalize the value of coin and pc amount
    pub sys_decimal_value: u64,
    /// All fee information
    pub fees: Fees,
    /// Statistical data
    pub state_data: StateData,
    /// Coin vault
    pub coin_vault: [u8; 32],
    /// Pc vault
    pub pc_vault: [u8; 32],
    /// Coin vault mint
    pub coin_vault_mint: [u8; 32],
    /// Pc vault mint
    pub pc_vault_mint: [u8; 32],
    /// lp mint
    pub lp_mint: [u8; 32],
    /// open_orders key
    pub open_orders: [u8; 32],
    /// market key
    pub market: [u8; 32],
    /// market program key
    pub market_program: [u8; 32],
    /// target_orders key
    pub target_orders: [u8; 32],
    /// padding
    pub padding1: [u64; 8],
    /// amm owner key
    pub amm_owner: [u8; 32],
    /// pool lp amount
    pub lp_amount: u64,
    /// client order id
    pub client_order_id: u64,
    /// recent epoch
    pub recent_epoch: u64,
    /// padding
    pub padding2: u64,
}

impl ProtocolEntity for AmmInfo {
    const PROGRAM_ID: Pubkey = RAYDIUM_AMM_ID;
    const DISCRIMINATOR: &'static [u8] = &[];
    const DATA_SIZE: usize = 752;

    fn deserialize(data: &[u8]) -> Option<Self> {
        Self::deserialize_bytemuck(data)
    }
}

impl DexPool for AmmInfo {
    fn get_mint_a(&self) -> Pubkey {
        Pubkey::from(self.coin_vault_mint)
    }

    fn get_mint_b(&self) -> Pubkey {
        Pubkey::from(self.pc_vault_mint)
    }

    fn get_vault_pubkeys(&self) -> Option<(Pubkey, Pubkey)> {
        Some((Pubkey::from(self.coin_vault), Pubkey::from(self.pc_vault)))
    }

    fn quote(&self, ctx: &QuoteContext) -> anyhow::Result<QuoteResult> {
        let (vault_coin, vault_pc) = ctx
            .vaults
            .ok_or_else(|| anyhow::anyhow!("Missing vault amounts for Raydium AMM"))?;

        let (total_pc_without_take_pnl, total_coin_without_take_pnl) =
            Calculator::calc_total_without_take_pnl_no_orderbook(vault_pc, vault_coin, self)?;

        let swap_direction = SwapDirection::from(ctx.a_to_b);

        match ctx.quote_type {
            QuoteType::ExactIn(amount) => {
                let amount_u128 = U128::from(amount);

                let swap_fee = amount_u128
                    .checked_mul(U128::from(self.fees.swap_fee_numerator))
                    .ok_or_else(|| anyhow::anyhow!("fee overflow"))?
                    .checked_ceil_div(U128::from(self.fees.swap_fee_denominator))
                    .ok_or_else(|| anyhow::anyhow!("fee ceil_div overflow"))?;

                let swap_in_after_deduct_fee = amount_u128
                    .checked_sub(swap_fee)
                    .ok_or_else(|| anyhow::anyhow!("fee exceeds amount"))?;

                let amount_out = Calculator::swap_token_amount_base_in(
                    swap_in_after_deduct_fee,
                    total_pc_without_take_pnl.into(),
                    total_coin_without_take_pnl.into(),
                    swap_direction,
                );

                Ok(QuoteResult {
                    steps: vec![],
                    total_amount_in_gross: amount,
                    total_amount_in_net: swap_in_after_deduct_fee.as_u64(),
                    total_amount_out: amount_out.as_u64(),
                    total_fee: swap_fee.as_u64(),
                    compute_units: AMM_COMPUTE_UNITS,
                })
            }

            QuoteType::ExactOut(amount) => {
                let swap_in_before_add_fee = Calculator::swap_token_amount_base_out(
                    amount.into(),
                    total_pc_without_take_pnl.into(),
                    total_coin_without_take_pnl.into(),
                    swap_direction,
                );

                // swap_in_after_add_fee * (1 - 0.0025) = swap_in_before_add_fee
                // swap_in_after_add_fee = swap_in_before_add_fee / (1 - 0.0025)
                let swap_in_after_add_fee = swap_in_before_add_fee
                    .checked_mul(self.fees.swap_fee_denominator.into())
                    .ok_or_else(|| anyhow::anyhow!("fee overflow"))?
                    .checked_ceil_div(
                        self.fees
                            .swap_fee_denominator
                            .checked_sub(self.fees.swap_fee_numerator)
                            .ok_or_else(|| anyhow::anyhow!("fee denominator underflow"))?
                            .into(),
                    )
                    .ok_or_else(|| anyhow::anyhow!("fee ceil_div overflow"))?
                    .as_u64();

                let swap_fee = swap_in_after_add_fee
                    .checked_sub(swap_in_before_add_fee.as_u64())
                    .ok_or_else(|| anyhow::anyhow!("fee underflow"))?;

                Ok(QuoteResult {
                    steps: vec![],
                    total_amount_in_gross: swap_in_after_add_fee,
                    total_amount_in_net: swap_in_before_add_fee.as_u64(),
                    total_amount_out: amount,
                    total_fee: swap_fee,
                    compute_units: AMM_COMPUTE_UNITS,
                })
            }
        }
    }
}

impl ProtocolMetrics for AmmInfo {
    fn name(&self) -> &'static str {
        DEX_RAYDIUM_AMM
    }
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct Fees {
    /// numerator of the min_separate
    pub min_separate_numerator: u64,
    /// denominator of the min_separate
    pub min_separate_denominator: u64,

    /// numerator of the fee
    pub trade_fee_numerator: u64,
    /// denominator of the fee
    /// and 'trade_fee_denominator' must be equal to 'min_separate_denominator'
    pub trade_fee_denominator: u64,

    /// numerator of the pnl
    pub pnl_numerator: u64,
    /// denominator of the pnl
    pub pnl_denominator: u64,

    /// numerator of the swap_fee
    pub swap_fee_numerator: u64,
    /// denominator of the swap_fee
    pub swap_fee_denominator: u64,
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct StateData {
    /// delay to take pnl coin
    pub need_take_pnl_coin: u64,
    /// delay to take pnl pc
    pub need_take_pnl_pc: u64,
    /// total pnl pc
    pub total_pnl_pc: u64,
    /// total pnl coin
    pub total_pnl_coin: u64,
    /// ido pool open time
    pub pool_open_time: u64,
    /// padding for future updates
    pub padding: [u64; 2],
    /// switch from orderbookonly to init
    pub orderbook_to_init_time: u64,

    /// swap coin in amount
    pub swap_coin_in_amount: [u64; 2],
    /// swap pc out amount
    pub swap_pc_out_amount: [u64; 2],
    /// charge pc as swap fee while swap pc to coin
    pub swap_acc_pc_fee: u64,

    /// swap pc in amount
    pub swap_pc_in_amount: [u64; 2],
    /// swap coin out amount
    pub swap_coin_out_amount: [u64; 2],
    /// charge coin as swap fee while swap coin to pc
    pub swap_acc_coin_fee: u64,
}
