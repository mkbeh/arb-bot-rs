use ahash::AHashMap;
use solana_sdk::pubkey::Pubkey;

use crate::libs::solana_client::dex::registry::{
    DexEntity,
    traits::{DexParser, RegistryLookup, ToDexParser},
};

/// Represents a single entry in the registry, combining a human-readable name
/// and the corresponding parser logic.
pub struct RegistryItem {
    pub name: &'static str,
    pub parser: DexParser,
}

pub struct DexRegistry {
    pub map: AHashMap<RegistryLookup, RegistryItem>,
}

impl DexRegistry {
    /// Creates a new, empty registry instance.
    pub fn new() -> Self {
        Self {
            map: AHashMap::new(),
        }
    }

    /// Registers a new entity and its parser into the registry.
    pub fn add<T, Out>(&mut self, wrap: fn(T) -> Out)
    where
        T: DexEntity + 'static,
        Out: ToDexParser<T> + 'static,
    {
        let lookup = Out::create_lookup();
        let parse_fn =
            move |data: &[u8]| -> Option<Out> { T::parse_into(data, |b: Box<T>| wrap(*b)) };

        self.map.insert(
            lookup,
            RegistryItem {
                name: std::any::type_name::<T>(),
                parser: Out::wrap_parser(parse_fn),
            },
        );
    }

    /// Retrieves a registry item based on the program ID and account data size.
    pub fn get_account_item(&self, program_id: &Pubkey, size: usize) -> Option<&RegistryItem> {
        let lookup = RegistryLookup::Account {
            program_id: *program_id,
            size,
        };
        self.map.get(&lookup)
    }

    /// Finds the best matching registry item for a given instruction payload.
    ///
    /// Uses the longest discriminator match to ensure accuracy when multiple
    /// instructions might share a similar prefix.
    pub fn get_instruction_item(
        &self,
        program_id: &Pubkey,
        payload: &[u8],
    ) -> Option<&RegistryItem> {
        self.map
            .iter()
            .filter_map(|(lookup, item)| match lookup {
                RegistryLookup::Instruction {
                    program_id: pid,
                    discriminator,
                } if pid == program_id && payload.starts_with(discriminator) => {
                    Some((discriminator.len(), item))
                }
                _ => None,
            })
            // Select the most specific match (longest discriminator)
            .max_by_key(|(len, _)| *len)
            .map(|(_, item)| item)
    }

    /// Returns all registered lookups and items associated with a specific Program ID.
    pub fn get_all_by_program_id(
        &self,
        program_id: &Pubkey,
    ) -> Vec<(&RegistryLookup, &RegistryItem)> {
        self.map
            .iter()
            .filter(|(lookup, _)| match lookup {
                RegistryLookup::Account {
                    program_id: pid, ..
                } => pid == program_id,
                RegistryLookup::Instruction {
                    program_id: pid, ..
                } => pid == program_id,
            })
            .collect()
    }
}
