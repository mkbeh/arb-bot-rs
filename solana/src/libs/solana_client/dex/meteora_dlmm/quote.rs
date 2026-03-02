use anyhow::{Context, Result, ensure};
use solana_sdk::pubkey::Pubkey;

use crate::libs::solana_client::dex::meteora_dlmm::{
    account::*, extensions::*, token_2022::calculate_transfer_fee_excluded_amount, types::*,
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

pub fn quote_exact_out() {
    todo!()
}

pub fn quote_exact_in(
    lb_pair_pubkey: Pubkey,
    lb_pair: &LbPair,
    amount_in: u64,
    swap_for_y: bool,
    bin_arrays: HashMap<Pubkey, BinArray>,
    bitmap_extension: Option<&BinArrayBitmapExtension>,
    clock: &Clock,
    mint_x_account: &Account,
    mint_y_account: &Account,
) -> Result<SwapExactInQuote> {
    todo!()
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
