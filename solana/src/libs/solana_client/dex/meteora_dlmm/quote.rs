use std::collections::BTreeMap;

use anyhow::{Context, Result, ensure};
use solana_sdk::{account::Account, clock::Clock};

use crate::libs::solana_client::{
    dex::meteora_dlmm::{account::*, extensions::*, token_2022::*, typedefs::*},
    pool::*,
};

const CU_BASE: u32 = 50_500;
const CU_PER_BIN: u32 = 150;
const CU_PER_ARRAY: u32 = 450;
const CU_PER_SWAP: u32 = 5_250;

pub fn validate_swap_activation(
    lb_pair: &LbPair,
    current_timestamp: u64,
    current_slot: u64,
) -> Result<()> {
    ensure!(
        lb_pair.status()?.eq(&PairStatus::Enabled),
        "Pair is disabled"
    );

    let pair_type = lb_pair.pair_type()?;
    if pair_type.eq(&PairType::Permission) {
        let activation_type = lb_pair.activation_type()?;
        let current_point = match activation_type {
            ActivationType::Slot => current_slot,
            ActivationType::Timestamp => current_timestamp,
        };

        ensure!(
            current_point >= lb_pair.activation_point,
            "Pair is disabled"
        );
    }

    Ok(())
}

pub fn shift_active_bin_if_empty_gap(
    lb_pair: &mut LbPair,
    active_bin_array: &BinArray,
    swap_for_y: bool,
) -> Result<()> {
    let lb_pair_bin_array_index = BinArray::bin_id_to_bin_array_index(lb_pair.active_id)?;

    if i64::from(lb_pair_bin_array_index) != active_bin_array.index {
        if swap_for_y {
            let (_, upper_bin_id) =
                BinArray::get_bin_array_lower_upper_bin_id(active_bin_array.index as i32)?;
            lb_pair.active_id = upper_bin_id;
        } else {
            let (lower_bin_id, _) =
                BinArray::get_bin_array_lower_upper_bin_id(active_bin_array.index as i32)?;
            lb_pair.active_id = lower_bin_id;
        }
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn quote_exact_out(
    lb_pair: &LbPair,
    mut amount_out: u64,
    swap_for_y: bool,
    bin_arrays: &BTreeMap<i64, BinArray>,
    bitmap_extension: Option<&BinArrayBitmapExtension>,
    clock: &Clock,
    mint_x_account: &Account,
    mint_y_account: &Account,
) -> Result<QuoteResult> {
    let current_timestamp = clock.unix_timestamp as u64;
    let current_slot = clock.slot;
    let epoch = clock.epoch;

    validate_swap_activation(lb_pair, current_timestamp, current_slot)?;

    let mut lb_pair = *lb_pair;
    lb_pair.update_references(current_timestamp as i64)?;

    let (in_mint_account, out_mint_account) = if swap_for_y {
        (mint_x_account, mint_y_account)
    } else {
        (mint_y_account, mint_x_account)
    };

    amount_out =
        calculate_transfer_fee_included_amount(out_mint_account, amount_out, epoch)?.amount;

    let mut total_amount_in: u64 = 0;
    let mut total_amount_out_net: u64 = 0;
    let mut total_fee: u64 = 0;
    let mut compute_units: u32 = 0;
    let mut steps = vec![];

    while amount_out > 0 {
        let Ok(array_index) = get_next_bin_array_index(&lb_pair, swap_for_y, bitmap_extension)
        else {
            break;
        };

        let mut active_bin_array = match bin_arrays.get(&array_index) {
            Some(arr) => *arr,
            None => break,
        };

        compute_units += CU_PER_ARRAY;

        shift_active_bin_if_empty_gap(&mut lb_pair, &active_bin_array, swap_for_y)?;

        loop {
            if !active_bin_array.is_bin_id_within_range(lb_pair.active_id)? || amount_out == 0 {
                break;
            }

            lb_pair.update_volatility_accumulator()?;

            let active_bin = active_bin_array.get_bin_mut(lb_pair.active_id)?;
            let price = active_bin.get_or_store_bin_price(lb_pair.active_id, lb_pair.bin_step)?;

            compute_units += CU_PER_BIN;

            if !active_bin.is_empty(!swap_for_y) {
                let bin_max_amount_out = active_bin.get_max_amount_out(swap_for_y);
                if amount_out >= bin_max_amount_out {
                    let max_amount_in = active_bin.get_max_amount_in(price, swap_for_y)?;
                    let max_fee = lb_pair.compute_fee(max_amount_in)?;

                    total_amount_in = total_amount_in
                        .checked_add(max_amount_in)
                        .context("MathOverflow")?;

                    total_amount_out_net = total_amount_out_net
                        .checked_add(bin_max_amount_out)
                        .context("MathOverflow")?;

                    total_fee = total_fee.checked_add(max_fee).context("MathOverflow")?;

                    amount_out = amount_out
                        .checked_sub(bin_max_amount_out)
                        .context("MathOverflow")?;

                    steps.push(QuoteSwapResult {
                        pool_state_id: lb_pair.active_id,
                        amount_in: max_amount_in + max_fee,
                        amount_out: bin_max_amount_out,
                        fee: max_fee,
                        price,
                    });
                } else {
                    let amount_in = Bin::get_amount_in(amount_out, price, swap_for_y)?;
                    let fee = lb_pair.compute_fee(amount_in)?;

                    total_amount_in = total_amount_in
                        .checked_add(amount_in)
                        .context("MathOverflow")?;

                    total_amount_out_net = total_amount_out_net
                        .checked_add(amount_out)
                        .context("MathOverflow")?;

                    total_fee = total_fee.checked_add(fee).context("MathOverflow")?;

                    steps.push(QuoteSwapResult {
                        pool_state_id: lb_pair.active_id,
                        amount_in: amount_in + fee,
                        amount_out,
                        fee,
                        price,
                    });

                    amount_out = 0;
                }

                compute_units += CU_PER_SWAP;
            }

            if amount_out > 0 {
                lb_pair.advance_active_bin(swap_for_y)?;
            }
        }
    }

    total_amount_in = total_amount_in
        .checked_add(total_fee)
        .context("MathOverflow")?;

    let total_amount_in_net = total_amount_in;

    let total_amount_in_gross =
        calculate_transfer_fee_included_amount(in_mint_account, total_amount_in, epoch)?.amount;

    let total_amount_out =
        calculate_transfer_fee_excluded_amount(out_mint_account, total_amount_out_net, epoch)?
            .amount;

    let compute_units = CU_BASE + compute_units;

    Ok(QuoteResult {
        steps,
        total_amount_in_gross,
        total_amount_in_net,
        total_amount_out,
        total_fee,
        compute_units,
    })
}

#[allow(clippy::too_many_arguments)]
pub fn quote_exact_in(
    lb_pair: &LbPair,
    amount_in: u64,
    swap_for_y: bool,
    bin_arrays: &BTreeMap<i64, BinArray>,
    bitmap_extension: Option<&BinArrayBitmapExtension>,
    clock: &Clock,
    mint_x_account: &Account,
    mint_y_account: &Account,
) -> Result<QuoteResult> {
    let current_timestamp = clock.unix_timestamp as u64;
    let current_slot = clock.slot;
    let epoch = clock.epoch;

    validate_swap_activation(lb_pair, current_timestamp, current_slot)?;

    let mut lb_pair = *lb_pair;
    lb_pair.update_references(current_timestamp as i64)?;

    let (in_mint_account, out_mint_account) = if swap_for_y {
        (mint_x_account, mint_y_account)
    } else {
        (mint_y_account, mint_x_account)
    };

    let transfer_fee_excluded_amount_in =
        calculate_transfer_fee_excluded_amount(in_mint_account, amount_in, epoch)?.amount;

    let mut amount_left = transfer_fee_excluded_amount_in;
    let mut total_amount_in_net: u64 = 0;
    let mut total_amount_out: u64 = 0;
    let mut total_fee: u64 = 0;
    let mut compute_units: u32 = 0;
    let mut steps = vec![];

    while amount_left > 0 {
        let Ok(array_index) = get_next_bin_array_index(&lb_pair, swap_for_y, bitmap_extension)
        else {
            break;
        };

        let mut active_bin_array = match bin_arrays.get(&array_index) {
            Some(arr) => *arr,
            None => break,
        };

        compute_units += CU_PER_ARRAY;

        shift_active_bin_if_empty_gap(&mut lb_pair, &active_bin_array, swap_for_y)?;

        loop {
            if !active_bin_array.is_bin_id_within_range(lb_pair.active_id)? || amount_left == 0 {
                break;
            }

            lb_pair.update_volatility_accumulator()?;

            let active_bin = active_bin_array.get_bin_mut(lb_pair.active_id)?;
            let price = active_bin.get_or_store_bin_price(lb_pair.active_id, lb_pair.bin_step)?;

            compute_units += CU_PER_BIN;

            if !active_bin.is_empty(!swap_for_y) {
                let SwapResult {
                    amount_in_with_fees,
                    amount_out,
                    fee,
                    ..
                } = active_bin.swap(amount_left, price, swap_for_y, &lb_pair, None)?;

                amount_left = amount_left
                    .checked_sub(amount_in_with_fees)
                    .context("MathOverflow")?;

                total_amount_in_net = total_amount_in_net
                    .checked_add(amount_in_with_fees)
                    .context("MathOverflow")?;

                total_amount_out = total_amount_out
                    .checked_add(amount_out)
                    .context("MathOverflow")?;
                total_fee = total_fee.checked_add(fee).context("MathOverflow")?;

                compute_units += CU_PER_SWAP;

                steps.push(QuoteSwapResult {
                    pool_state_id: lb_pair.active_id,
                    amount_in: amount_in_with_fees,
                    amount_out,
                    fee,
                    price,
                });
            }

            if amount_left > 0 {
                lb_pair.advance_active_bin(swap_for_y)?;
            }
        }
    }

    let total_amount_in_gross =
        calculate_transfer_fee_included_amount(in_mint_account, total_amount_in_net, epoch)?.amount;

    let total_amount_out =
        calculate_transfer_fee_excluded_amount(out_mint_account, total_amount_out, epoch)?.amount;

    let compute_units = CU_BASE + compute_units;

    Ok(QuoteResult {
        steps,
        total_fee,
        total_amount_in_gross,
        total_amount_in_net,
        total_amount_out,
        compute_units,
    })
}

pub fn get_next_bin_array_index(
    lb_pair: &LbPair,
    swap_for_y: bool,
    bitmap_extension: Option<&BinArrayBitmapExtension>,
) -> Result<i64> {
    let start_array_idx = BinArray::bin_id_to_bin_array_index(lb_pair.active_id)?;

    // Logic from get_bin_array_pubkeys_for_swap but without PDA derivation and Vecs
    let (next_idx, has_liquidity) = if lb_pair.is_overflow_default_bin_array_bitmap(start_array_idx)
    {
        match bitmap_extension {
            Some(ext) => ext.next_bin_array_index_with_liquidity(swap_for_y, start_array_idx)?,
            None => (0, false),
        }
    } else {
        lb_pair.next_bin_array_index_with_liquidity_internal(swap_for_y, start_array_idx)?
    };

    if has_liquidity {
        Ok(next_idx as i64)
    } else {
        anyhow::bail!("Liquidity exhausted in bitmap")
    }
}
