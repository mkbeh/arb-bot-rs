use solana_sdk::program_pack::Pack;
use spl_token_2022::extension::{
    BaseState, BaseStateWithExtensions, StateWithExtensions, transfer_fee::*,
};

/// Calculate the fee for output amount
pub fn get_transfer_inverse_fee<S: BaseState + Pack>(
    account_state: &StateWithExtensions<S>,
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
pub fn get_transfer_fee<S: BaseState + Pack>(
    account_state: &StateWithExtensions<S>,
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
