use anyhow::{Context, Result, bail, ensure};
use num_integer::Integer;

use crate::libs::solana_client::dex::meteora_dlmm::{account::*, constants::*};

pub trait BinArrayExtension {
    fn is_bin_id_within_range(&self, bin_id: i32) -> Result<bool>;
    fn get_bin_index_in_array(&self, bin_id: i32) -> Result<usize>;

    fn get_bin_array_lower_upper_bin_id(index: i32) -> Result<(i32, i32)>;
    fn bin_id_to_bin_array_index(bin_id: i32) -> Result<i32>;

    fn get_bin_mut(&mut self, bin_id: i32) -> Result<&mut Bin>;
    fn get_bin(&self, bin_id: i32) -> Result<&Bin>;
}

impl BinArrayExtension for BinArray {
    fn is_bin_id_within_range(&self, bin_id: i32) -> Result<bool> {
        let (lower_bin_id, upper_bin_id) =
            Self::get_bin_array_lower_upper_bin_id(self.index as i32)?;

        Ok(bin_id >= lower_bin_id && bin_id <= upper_bin_id)
    }

    fn get_bin_index_in_array(&self, bin_id: i32) -> Result<usize> {
        ensure!(self.is_bin_id_within_range(bin_id)?, "Bin id out of range");
        let (lower_bin_id, _) = Self::get_bin_array_lower_upper_bin_id(self.index as i32)?;
        let index = bin_id.checked_sub(lower_bin_id).context("overflow")?;
        Ok(index as usize)
    }

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

    fn get_bin_mut(&mut self, bin_id: i32) -> Result<&mut Bin> {
        let idx = self.get_bin_index_in_array(bin_id)?;
        match idx {
            0..=31 => Ok(&mut self.bins_1[idx]),
            32..=63 => Ok(&mut self.bins_2[idx - 32]),
            64..=69 => Ok(&mut self.bins_3[idx - 64]),
            _ => bail!("Bin index {idx} is out of range"),
        }
    }

    fn get_bin(&self, bin_id: i32) -> Result<&Bin> {
        let idx = self.get_bin_index_in_array(bin_id)?;
        match idx {
            0..=31 => Ok(&self.bins_1[idx]),
            32..=63 => Ok(&self.bins_2[idx - 32]),
            64..=69 => Ok(&self.bins_3[idx - 64]),
            _ => bail!("index out of range"),
        }
    }
}
