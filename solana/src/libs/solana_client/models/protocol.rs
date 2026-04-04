use std::collections::HashMap;

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
