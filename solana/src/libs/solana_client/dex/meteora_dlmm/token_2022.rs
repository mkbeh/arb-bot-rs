use anyhow::{Context, Result};
use solana_sdk::account::Account;
use spl_token_2022::extension::{
    transfer_fee::{TransferFee, TransferFeeConfig, *},
    *,
};

const ONE_IN_BASIS_POINTS: u128 = MAX_FEE_BASIS_POINTS as u128;

pub fn get_epoch_transfer_fee(mint_account: &Account, epoch: u64) -> Result<Option<TransferFee>> {
    if mint_account.owner == spl_token::ID {
        return Ok(None);
    }

    let token_mint_data = mint_account.data.as_ref();
    let token_mint_unpacked =
        StateWithExtensions::<spl_token_2022::state::Mint>::unpack(token_mint_data)?;

    if let Ok(transfer_fee_config) = token_mint_unpacked.get_extension::<TransferFeeConfig>() {
        return Ok(Some(*transfer_fee_config.get_epoch_fee(epoch)));
    }

    Ok(None)
}

#[derive(Debug)]
pub struct TransferFeeExcludedAmount {
    pub amount: u64,
    pub transfer_fee: u64,
}

pub fn calculate_transfer_fee_excluded_amount(
    mint_account: &Account,
    transfer_fee_included_amount: u64,
    epoch: u64,
) -> Result<TransferFeeExcludedAmount> {
    if let Some(epoch_transfer_fee) = get_epoch_transfer_fee(mint_account, epoch)? {
        let transfer_fee = epoch_transfer_fee
            .calculate_fee(transfer_fee_included_amount)
            .context("MathOverflow")?;
        let transfer_fee_excluded_amount = transfer_fee_included_amount
            .checked_sub(transfer_fee)
            .context("MathOverflow")?;

        return Ok(TransferFeeExcludedAmount {
            amount: transfer_fee_excluded_amount,
            transfer_fee,
        });
    }

    Ok(TransferFeeExcludedAmount {
        amount: transfer_fee_included_amount,
        transfer_fee: 0,
    })
}

#[derive(Debug)]
pub struct TransferFeeIncludedAmount {
    pub amount: u64,
    pub transfer_fee: u64,
}

pub fn calculate_transfer_fee_included_amount(
    mint_account: &Account,
    transfer_fee_excluded_amount: u64,
    epoch: u64,
) -> Result<TransferFeeIncludedAmount> {
    if transfer_fee_excluded_amount == 0 {
        return Ok(TransferFeeIncludedAmount {
            amount: 0,
            transfer_fee: 0,
        });
    }

    if let Some(epoch_transfer_fee) = get_epoch_transfer_fee(mint_account, epoch)? {
        let transfer_fee: u64 =
            if u16::from(epoch_transfer_fee.transfer_fee_basis_points) == MAX_FEE_BASIS_POINTS {
                u64::from(epoch_transfer_fee.maximum_fee)
            } else {
                calculate_inverse_fee(&epoch_transfer_fee, transfer_fee_excluded_amount)
                    .context("MathOverflow")?
            };

        let transfer_fee_included_amount = transfer_fee_excluded_amount
            .checked_add(transfer_fee)
            .context("MathOverflow")?;

        return Ok(TransferFeeIncludedAmount {
            amount: transfer_fee_included_amount,
            transfer_fee,
        });
    }

    Ok(TransferFeeIncludedAmount {
        amount: transfer_fee_excluded_amount,
        transfer_fee: 0,
    })
}

#[must_use]
pub fn calculate_pre_fee_amount(transfer_fee: &TransferFee, post_fee_amount: u64) -> Option<u64> {
    if post_fee_amount == 0 {
        return Some(0);
    }
    let maximum_fee = u64::from(transfer_fee.maximum_fee);
    let transfer_fee_basis_points = u16::from(transfer_fee.transfer_fee_basis_points) as u128;
    if transfer_fee_basis_points == 0 {
        Some(post_fee_amount)
    } else if transfer_fee_basis_points == ONE_IN_BASIS_POINTS {
        Some(maximum_fee.checked_add(post_fee_amount)?)
    } else {
        let numerator = (post_fee_amount as u128).checked_mul(ONE_IN_BASIS_POINTS)?;
        let denominator = ONE_IN_BASIS_POINTS.checked_sub(transfer_fee_basis_points)?;
        let raw_pre_fee_amount = numerator
            .checked_add(denominator)?
            .checked_sub(1)?
            .checked_div(denominator)?;

        if raw_pre_fee_amount.checked_sub(post_fee_amount as u128)? >= maximum_fee as u128 {
            post_fee_amount.checked_add(maximum_fee)
        } else {
            u64::try_from(raw_pre_fee_amount).ok()
        }
    }
}

#[must_use]
pub fn calculate_inverse_fee(transfer_fee: &TransferFee, post_fee_amount: u64) -> Option<u64> {
    let pre_fee_amount = calculate_pre_fee_amount(transfer_fee, post_fee_amount)?;
    transfer_fee.calculate_fee(pre_fee_amount)
}
