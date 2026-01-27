use solana_sdk::pubkey::Pubkey;

use crate::libs::solana_client::dex::{
    models::{PoolState, TxEvent},
    parser::DexEntity,
};

/// Defines the criteria used to locate a specific parser in the registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RegistryLookup {
    /// Lookup for account data based on the owning program and expected data size.
    Account { program_id: Pubkey, size: usize },
    /// Lookup for transaction instructions based on the program and a unique byte discriminator.
    Instruction {
        program_id: Pubkey,
        /// The unique prefix (e.g., Anchor discriminator) used to identify the instruction type.
        discriminator: &'static [u8],
    },
}

impl RegistryLookup {
    /// Returns the Solana Program ID associated with this lookup entry.
    pub fn program_id(&self) -> Pubkey {
        match self {
            Self::Account { program_id, .. } => *program_id,
            Self::Instruction { program_id, .. } => *program_id,
        }
    }
}

/// A type alias for a parsing function.
/// Takes raw bytes as input and returns an optional deserialized object.
pub type ParserFn<T> = Box<dyn Fn(&[u8]) -> Option<T> + Send + Sync + 'static>;

/// Container for different types of DEX parsers.
pub enum DexParser {
    Account(ParserFn<PoolState>),
    Tx(ParserFn<TxEvent>),
}

/// A bridge trait that links a raw DEX entity (from a specific protocol)
/// to the internal registry's parsing system.
///
/// - `T`: The raw entity type (must implement `DexEntity`)
pub trait ToDexParser<T: DexEntity>: Sized {
    /// Generates the appropriate `RegistryLookup` for the given entity.
    fn create_lookup() -> RegistryLookup;

    /// Wraps a specialized parsing closure into a generic `DexParser` enum.
    fn wrap_parser<F>(f: F) -> DexParser
    where
        F: Fn(&[u8]) -> Option<Self> + Send + Sync + 'static;
}
