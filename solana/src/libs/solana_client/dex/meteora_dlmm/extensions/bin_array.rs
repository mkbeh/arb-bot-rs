use anyhow::{Context, Result};

use crate::libs::solana_client::dex::meteora_dlmm::{account::*, constants::*};

pub trait BinArrayExtension {
    fn get_bin_array_lower_upper_bin_id(index: i32) -> Result<(i32, i32)>;
    fn bin_id_to_bin_array_index(bin_id: i32) -> Result<i32>;
}

impl BinArrayExtension for BinArray {
    fn get_bin_array_lower_upper_bin_id(index: i32) -> Result<(i32, i32)> {
        let lower_bin_id = index
            .checked_mul(MAX_BIN_PER_ARRAY as i32)
            .context("overflow")?;

        let upper_bin_id = lower_bin_id
            .checked_add(MAX_BIN_PER_ARRAY as i32)
            .context("overflow")?
            .checked_sub(1)
            .context("overflow")?;

        Ok((lower_bin_id, upper_bin_id))
    }

    fn bin_id_to_bin_array_index(bin_id: i32) -> Result<i32> {
        let (idx, rem) = bin_id.div_rem(&(MAX_BIN_PER_ARRAY as i32));

        if bin_id.is_negative() && rem != 0 {
            Ok(idx.checked_sub(1).context("overflow")?)
        } else {
            Ok(idx)
        }
    }
}
