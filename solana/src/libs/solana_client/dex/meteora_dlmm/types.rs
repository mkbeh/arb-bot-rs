#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rounding {
    Up,
    Down,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ActivationType {
    Slot,
    Timestamp,
}

#[derive(Clone, Debug, PartialEq)]
#[repr(u8)]
pub enum PairType {
    Permissionless,
    Permission,
    CustomizablePermissionless,
    PermissionlessV2,
}
#[derive(Clone, Debug, PartialEq)]
#[repr(u8)]
pub enum PairStatus {
    Enabled,
    Disabled,
}
