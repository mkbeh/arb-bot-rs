use ahash::AHashMap;
use anyhow::anyhow;
use solana_sdk::pubkey::Pubkey;

use crate::libs::solana_client::registry::{
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

impl Default for DexRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl DexRegistry {
    /// Creates a new, empty registry instance.
    #[must_use]
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

    /// Registers a new entity and its parser into the registry.
    pub fn add_boxed<T, Out>(&mut self, wrap: fn(Box<T>) -> Out)
    where
        T: DexEntity + 'static,
        Out: ToDexParser<T> + 'static,
    {
        let lookup = Out::create_lookup();

        // Передаем wrap напрямую, так как он ожидает Box<T>,
        // а parse_into как раз предоставляет Box<T>
        let parse_fn = move |data: &[u8]| -> Option<Out> { T::parse_into(data, wrap) };

        self.map.insert(
            lookup,
            RegistryItem {
                name: std::any::type_name::<T>(),
                parser: Out::wrap_parser(parse_fn),
            },
        );
    }

    /// Retrieves a registry item based on the program ID and account data size.
    #[must_use]
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
    #[must_use]
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
    #[must_use]
    pub fn get_all_by_program_id(
        &self,
        program_id: &Pubkey,
    ) -> Vec<(&RegistryLookup, &RegistryItem)> {
        self.map
            .iter()
            .filter(|(lookup, _)| match lookup {
                RegistryLookup::Account {
                    program_id: pid, ..
                }
                | RegistryLookup::Instruction {
                    program_id: pid, ..
                } => pid == program_id,
            })
            .collect()
    }

    /// Returns a list of registry entries for the provided program address strings.
    ///
    /// This method follows an "all-or-nothing" strategy: it short-circuits and returns
    /// an error if any string is not a valid `Pubkey` or if a program is missing from the registry.
    pub fn get_all_from_strings(
        &self,
        program_ids: &[String],
    ) -> anyhow::Result<Vec<(&RegistryLookup, &RegistryItem)>> {
        program_ids
            .iter()
            .map(|id| {
                let pk = id
                    .parse::<Pubkey>()
                    .map_err(|e| anyhow!("Invalid Pubkey {id}: {e}"))?;

                let entries = self.get_all_by_program_id(&pk);

                (!entries.is_empty())
                    .then_some(entries)
                    .ok_or_else(|| anyhow!("Program ID not found in registry: {pk}"))
            })
            .collect::<anyhow::Result<Vec<_>>>()
            .map(|vecs| vecs.into_iter().flatten().collect())
    }
}
