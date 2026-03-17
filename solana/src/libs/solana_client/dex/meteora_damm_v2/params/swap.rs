use num_enum::{IntoPrimitive, TryFromPrimitive};

/// Trade (swap) direction
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, IntoPrimitive, TryFromPrimitive)]
pub enum TradeDirection {
    /// Input token A, output token B
    AtoB,
    /// Input token B, output token A
    BtoA,
}

impl From<bool> for TradeDirection {
    fn from(a_to_b: bool) -> Self {
        if a_to_b { Self::AtoB } else { Self::BtoA }
    }
}
