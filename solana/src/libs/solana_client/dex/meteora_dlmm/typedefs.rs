use num_enum::{IntoPrimitive, TryFromPrimitive};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rounding {
    Up,
    Down,
}

#[derive(Clone, Debug, PartialEq, Eq, TryFromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub enum ActivationType {
    Slot,
    Timestamp,
}

#[derive(Clone, Debug, PartialEq, TryFromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub enum PairType {
    Permissionless,
    Permission,
    CustomizablePermissionless,
    PermissionlessV2,
}
#[derive(Clone, Debug, PartialEq, TryFromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub enum PairStatus {
    Enabled,
    Disabled,
}

#[derive(Debug)]
pub struct SwapResult {
    /// Amount of token swap into the bin
    pub amount_in_with_fees: u64,
    /// Amount of token swap out from the bin
    pub amount_out: u64,
    /// Swap fee, includes protocol fee
    pub fee: u64,
    /// Part of fee
    pub protocol_fee_after_host_fee: u64,
    /// Part of protocol fee
    pub host_fee: u64,
    /// Indicate whether we reach exact out amount
    pub is_exact_out_amount: bool,
}
