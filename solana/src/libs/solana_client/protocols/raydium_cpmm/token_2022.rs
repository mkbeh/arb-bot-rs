use bytemuck::Pod;
use spl_token_2022::extension::{
    BaseState, BaseStateWithExtensions, PodStateWithExtensions,
    transfer_fee::{MAX_FEE_BASIS_POINTS, TransferFeeConfig},
};

/// Calculate the fee for output amount
#[must_use]
pub fn get_transfer_inverse_fee<S: BaseState + Pod>(
    account_state: &PodStateWithExtensions<S>,
    epoch: u64,
    post_fee_amount: u64,
) -> u64 {
    if let Ok(transfer_fee_config) = account_state.get_extension::<TransferFeeConfig>() {
        let transfer_fee = transfer_fee_config.get_epoch_fee(epoch);
        if u16::from(transfer_fee.transfer_fee_basis_points) == MAX_FEE_BASIS_POINTS {
            u64::from(transfer_fee.maximum_fee)
        } else {
            transfer_fee_config
                .calculate_inverse_epoch_fee(epoch, post_fee_amount)
                .unwrap()
        }
    } else {
        0
    }
}

/// Calculate the fee for input amount
#[must_use]
pub fn get_transfer_fee<S: BaseState + Pod>(
    account_state: &PodStateWithExtensions<S>,
    epoch: u64,
    pre_fee_amount: u64,
) -> u64 {
    if let Ok(transfer_fee_config) = account_state.get_extension::<TransferFeeConfig>() {
        transfer_fee_config
            .calculate_epoch_fee(epoch, pre_fee_amount)
            .unwrap()
    } else {
        0
    }
}
