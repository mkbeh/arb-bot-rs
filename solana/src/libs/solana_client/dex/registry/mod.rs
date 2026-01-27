pub mod core;
pub mod definitions;
pub mod traits;

pub use core::{DexRegistry, RegistryItem};

pub use traits::{DexEntity, DexParser, RegistryLookup, ToDexParser};

/// Global, thread-safe registry that stores all supported DEX protocol parsers.
pub static DEX_REGISTRY: std::sync::LazyLock<DexRegistry> = std::sync::LazyLock::new(|| {
    let mut reg = DexRegistry::new();

    // Fill the registry with protocol-specific entities and their wrappers
    definitions::fill_registry(&mut reg);

    reg
});
