use tokio_util::sync::CancellationToken;

use crate::libs::solana_client::{Event, GrpcClient, GrpcConfig};

pub struct TxStream {
    grpc_config: GrpcConfig,
}

impl TxStream {
    pub fn new(endpoint: String, x_token: Option<String>, program_ids: Vec<String>) -> Self {
        let grpc_config = GrpcConfig {
            endpoint,
            x_token,
            program_ids,
            ..Default::default()
        };
        Self { grpc_config }
    }

    pub async fn start(&self, token: CancellationToken) -> anyhow::Result<()> {
        GrpcClient::new(self.grpc_config.clone())
            .with_callback(|event: Event| {
                println!("Got event {:?}", event);
                Ok(())
            })
            .subscribe(token)
            .await
    }
}
