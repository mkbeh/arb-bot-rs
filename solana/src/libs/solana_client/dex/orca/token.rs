use orca_whirlpools_core::TransferFee;
use spl_token_2022::extension::{
    BaseStateWithExtensions, StateWithExtensions, transfer_fee::TransferFeeConfig,
};

#[must_use]
pub fn get_epoch_transfer_fee(
    mint: &StateWithExtensions<spl_token_2022::state::Mint>,
    epoch: u64,
) -> Option<TransferFee> {
    mint.get_extension::<TransferFeeConfig>().ok().map(|c| {
        let fee = c.get_epoch_fee(epoch);
        TransferFee::new_with_max(
            u16::from(fee.transfer_fee_basis_points),
            u64::from(fee.maximum_fee),
        )
    })
}
