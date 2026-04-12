use crate::libs::solana_client::metrics::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolKind {
    MeteoraDammV2,
    MeteoraDlmm,
    RaydiumAmm,
    RaydiumClmm,
    RaydiumCpmm,
    Orca,
    PumpFun,
}

impl ProtocolKind {
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::MeteoraDammV2 => DEX_METEORA_DAMM_V2,
            Self::MeteoraDlmm => DEX_METEORA_DLMM,
            Self::RaydiumAmm => DEX_RAYDIUM_AMM,
            Self::RaydiumClmm => DEX_RAYDIUM_CLMM,
            Self::RaydiumCpmm => DEX_RAYDIUM_CPMM,
            Self::Orca => DEX_ORCA,
            Self::PumpFun => DEX_PUMP_FUN,
        }
    }
}

/// Trait for mandatory protocol metadata
pub trait ProtocolMetrics {
    fn protocol(&self) -> ProtocolKind;

    fn protocol_name(&self) -> &'static str {
        self.protocol().as_str()
    }
}
