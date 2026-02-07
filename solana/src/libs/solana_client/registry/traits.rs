use bytemuck::Pod;
use solana_sdk::pubkey::Pubkey;
use tracing::error;

use crate::libs::solana_client::models::{PoolState, TxEvent};

/// Defines the criteria used to locate a specific parser in the registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RegistryLookup {
    /// Lookup for account data based on the owning program and expected data size.
    Account {
        program_id: Pubkey,
        size: usize,
        discriminator: &'static [u8],
    },
    /// Lookup for transaction instructions based on the program and a unique byte discriminator.
    Instruction {
        program_id: Pubkey,
        /// The unique prefix (e.g., Anchor discriminator) used to identify the instruction type.
        discriminator: &'static [u8],
    },
}

impl RegistryLookup {
    /// Returns the Solana Program ID associated with this lookup entry.
    #[must_use]
    pub fn program_id(&self) -> Pubkey {
        match self {
            Self::Account { program_id, .. } | Self::Instruction { program_id, .. } => *program_id,
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

/// A core trait that defines the interface for any DEX-related blockchain entity.
///
/// This trait provides constants for identification and unified methods for
/// deserializing raw Solana data into structured types.
pub trait DexEntity: Sized {
    /// The unique public key of the Solana program that owns this entity.
    const PROGRAM_ID: Pubkey;

    /// The unique byte prefix (discriminator) used to identify the
    /// specific type (common in Anchor-based programs).
    const DISCRIMINATOR: &'static [u8];

    /// The expected fixed size of the data in bytes (0 to disable RPC dataSize filter).
    const DATA_SIZE: usize;

    /// Primary deserialization method to be implemented by each specific DEX type.
    fn deserialize(data: &[u8]) -> Option<Self>;

    /// This method validates the data length and checks for the correct discriminator
    /// before reinterpreting the raw byte buffer as a struct in memory.
    #[must_use]
    fn deserialize_bytemuck(data: &[u8]) -> Option<Self>
    where
        Self: Pod + Copy,
    {
        let type_name = std::any::type_name::<Self>();
        let disc_size = Self::DISCRIMINATOR.len();
        let expected_size = disc_size + size_of::<Self>();

        // Ensure buffer contains at least the required amount of bytes
        if data.len() != expected_size {
            error!(
                "[{type_name}] Size mismatch: expected exactly {expected_size}, got {}",
                data.len()
            );
            return None;
        }

        // Validate the type discriminator prefix
        if disc_size > 0 && !data.starts_with(Self::DISCRIMINATOR) {
            error!(
                "[{type_name}] Discriminator mismatch. Expected prefix {:?}, got {:?}",
                Self::DISCRIMINATOR,
                &data[..disc_size.min(data.len())]
            );
            return None;
        }

        // Access the payload slice after the discriminator and read it into the struct
        let payload = data.get(disc_size..)?;
        Some(bytemuck::pod_read_unaligned(payload))
    }

    /// Deserializes the data and maps the result into an output wrapper.
    fn parse_into<Out, F>(data: &[u8], wrap: F) -> Option<Out>
    where
        F: FnOnce(Box<Self>) -> Out,
    {
        Self::deserialize(data).map(|val| wrap(Box::new(val)))
    }
}
