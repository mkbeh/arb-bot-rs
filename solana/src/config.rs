use anyhow::bail;
use engine::Validatable;
use serde::Deserialize;

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Transport {
    Websocket,
    Grpc,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub transport: Transport,
    pub rpc_endpoint: String,
    pub grpc_endpoint: Option<String>,
    pub x_token: Option<String>,
    pub ws_endpoint: Option<String>,
    pub ws_api_key: Option<String>,
    pub exchanges: Vec<Dex>,
}

impl Validatable for Config {
    fn validate(&mut self) -> anyhow::Result<()> {
        if self.rpc_endpoint.is_empty() {
            bail!("RPC endpoint cannot be empty");
        }
        Ok(())
    }
}

impl Config {
    #[must_use]
    pub fn get_dex_programs(&self) -> Vec<String> {
        self.exchanges
            .iter()
            .map(|d| d.program_id.clone())
            .collect()
    }
}

#[derive(Deserialize, Clone, Debug)]
pub struct Dex {
    pub program_id: String,
}
