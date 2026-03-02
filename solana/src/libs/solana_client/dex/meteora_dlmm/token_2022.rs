use anyhow::{Context, Result};
use solana_sdk::account::Account;
use spl_token_2022::extension::{
    transfer_fee::{TransferFee, TransferFeeConfig},
    *,
};

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
