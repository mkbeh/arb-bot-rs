use std::collections::HashMap;

use solana_sdk::pubkey::Pubkey;

use crate::libs::solana_client::{
    metrics::{
        DEX_METEORA_DAMM_V2, DEX_METEORA_DLMM, DEX_ORCA, DEX_RAYDIUM_AMM, DEX_RAYDIUM_CLMM,
        DEX_RAYDIUM_CPMM,
    },
    protocols::{meteora_dlmm, raydium_clmm},
};

/// Configuration for a single protocol, including its program ID
/// and optional list of specific account addresses to subscribe to.
#[derive(Clone, Debug)]
pub struct ProtocolConfig {
    /// The on-chain program ID of the DEX protocol.
    pub program_id: String,
    /// Specific account addresses to subscribe to (e.g. lending reserves).
    /// Empty means subscribe to all accounts owned by the program.
    pub account_ids: Vec<String>,
}

/// A map of protocol configurations keyed by program ID.
/// Provides efficient lookup by program ID string.
#[derive(Clone, Debug, Default)]
pub struct ProtocolMap(HashMap<String, ProtocolConfig>);

impl ProtocolMap {
    #[must_use]
    pub fn get(&self, program_id: &str) -> Option<&ProtocolConfig> {
        self.0.get(program_id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &ProtocolConfig> {
        self.0.values()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl FromIterator<ProtocolConfig> for ProtocolMap {
    fn from_iter<I: IntoIterator<Item = ProtocolConfig>>(iter: I) -> Self {
        Self(
            iter.into_iter()
                .map(|p| (p.program_id.clone(), p))
                .collect(),
        )
    }
}

pub trait ProtocolIdentity {
    fn protocol(&self) -> ProtocolKind;

    fn protocol_name(&self) -> &'static str {
        self.protocol().as_str()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolKind {
    MeteoraDammV2,
    MeteoraDlmm,
    RaydiumAmm,
    RaydiumClmm,
    RaydiumCpmm,
    Orca,
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
        }
    }

    #[must_use]
    pub fn bitmap_pda(&self, pool_id: &Pubkey) -> Option<Pubkey> {
        match self {
            Self::MeteoraDlmm => Some(meteora_dlmm::derive_bin_array_bitmap_extension(*pool_id)),
            Self::RaydiumClmm => Some(raydium_clmm::derive_tick_array_bitmap_extension(*pool_id)),
            _ => None,
        }
    }
}
