use crate::libs::solana_client::{
    models::*,
    protocols::{
        raydium_clmm::AmmConfig as RaydiumClmmAmmConfig,
        raydium_cpmm::AmmConfig as RaydiumCpmmAmmConfig,
    },
};

/// A unified wrapper for various Automated Market Maker (AMM) configurations.
#[derive(Debug, Clone, Copy)]
pub enum AmmConfigType {
    Clmm(RaydiumClmmAmmConfig),
    Cpmm(RaydiumCpmmAmmConfig),
}

impl AmmConfigType {
    #[must_use]
    pub fn as_clmm(&self) -> Option<&RaydiumClmmAmmConfig> {
        match self {
            Self::Clmm(c) => Some(c),
            Self::Cpmm(_) => None,
        }
    }

    #[must_use]
    pub fn as_cpmm(&self) -> Option<&RaydiumCpmmAmmConfig> {
        match self {
            Self::Cpmm(c) => Some(c),
            Self::Clmm(_) => None,
        }
    }
}

impl From<RaydiumClmmAmmConfig> for AmmConfigType {
    fn from(c: RaydiumClmmAmmConfig) -> Self {
        Self::Clmm(c)
    }
}

impl From<RaydiumCpmmAmmConfig> for AmmConfigType {
    fn from(c: RaydiumCpmmAmmConfig) -> Self {
        Self::Cpmm(c)
    }
}

//  --- Entry trait ---

/// A marker trait for concrete AMM configuration types.
pub trait AmmConfigEntry: ProtocolIdentity + Into<AmmConfigType> + Copy {
    /// Attempts to extract a reference to the concrete type from the generic [`AmmConfigType`].
    fn extract(config: &AmmConfigType) -> Option<&Self>;
}

impl ProtocolIdentity for RaydiumClmmAmmConfig {
    fn protocol(&self) -> ProtocolKind {
        ProtocolKind::RaydiumClmm
    }
}

impl ProtocolIdentity for RaydiumCpmmAmmConfig {
    fn protocol(&self) -> ProtocolKind {
        ProtocolKind::RaydiumCpmm
    }
}

impl AmmConfigEntry for RaydiumClmmAmmConfig {
    fn extract(config: &AmmConfigType) -> Option<&Self> {
        config.as_clmm()
    }
}

impl AmmConfigEntry for RaydiumCpmmAmmConfig {
    fn extract(config: &AmmConfigType) -> Option<&Self> {
        config.as_cpmm()
    }
}
