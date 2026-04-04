pub mod core;
pub mod definitions;
pub mod traits;

pub use core::*;

pub use traits::*;

/// Global, thread-safe registry that stores all supported DEX protocol parsers.
pub static PROTOCOL_REGISTRY: std::sync::LazyLock<ProtocolRegistry> =
    std::sync::LazyLock::new(|| {
        let mut reg = ProtocolRegistry::default();

        // Fill the registry with protocol-specific entities and their wrappers
        definitions::fill_registry(&mut reg);

        reg
    });
