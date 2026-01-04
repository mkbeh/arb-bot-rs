use anyhow::bail;
use serde::Deserialize;

/// General application settings.
#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub rpc_endpoint: String,
    pub grpc_endpoint: String,
    pub x_token: Option<String>,
    pub exchanges: Vec<Dex>,
}

impl Config {
    /// Validates the configuration at startup.
    pub fn validate(&self) -> anyhow::Result<()> {
        if self.rpc_endpoint.is_empty() || self.grpc_endpoint.is_empty() {
            bail!("RPC or gRPC endpoint cannot be empty");
        }
        Ok(())
    }

    /// Extracts and returns a list of all DEX program IDs from the configured exchanges.
    #[must_use]
    pub fn get_dex_programs(&self) -> Vec<String> {
        self.exchanges
            .iter()
            .map(|d| d.program_id.clone())
            .collect()
    }
}

/// A single Decentralized Exchange (DEX) configuration.
#[derive(Deserialize, Clone, Debug)]
pub struct Dex {
    pub program_id: String,
}
