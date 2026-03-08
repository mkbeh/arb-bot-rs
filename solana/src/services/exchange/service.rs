use std::sync::Arc;

use anyhow::anyhow;
use async_trait::async_trait;
use engine::{Exchange, service::traits::ArbitrageService};
use tokio::{sync::Mutex, task::JoinSet};
use tokio_util::sync::CancellationToken;

use crate::{
    Config,
    config::Transport,
    libs::solana_client::{
        GrpcClient, GrpcConfig, RpcClient, SolanaStream, StreamClient, StreamConfig,
        models::SubscribeTarget,
    },
    services::exchange::{cache, market::MarketService, mint::MintService},
};

pub struct ExchangeService {
    mint_service: Arc<MintService>,
    market_stream: Arc<Mutex<Box<dyn SolanaStream>>>,
}

impl Exchange for ExchangeService {}

#[async_trait]
impl ArbitrageService for ExchangeService {
    async fn start(&self, token: CancellationToken) -> anyhow::Result<()> {
        let mut tasks_set = JoinSet::new();

        // Background job: periodically refreshes mint accounts cache via RPC
        tasks_set.spawn({
            let token = token.clone();
            let mint_service = self.mint_service.clone();

            async move { mint_service.start(token).await }
        });

        // Main market stream: subscribes to on-chain account updates via websocket/gRPC
        tasks_set.spawn({
            let token = token.clone();
            let stream = self.market_stream.clone();

            async move {
                let mut stream = stream.lock().await;
                stream.subscribe(token).await
            }
        });

        // If any task finishes (either completes or errors),
        // cancel all others and propagate result
        if let Some(result) = tasks_set.join_next().await {
            token.cancel();

            return result
                .map_err(|e| anyhow!("Task panicked: {e}"))
                .and_then(|result| result);
        }

        Ok(())
    }
}

impl ExchangeService {
    pub async fn from_config(config: &Config) -> anyhow::Result<Self> {
        cache::init(config.liquidity_depth)?;

        let rpc_client = Arc::new(RpcClient::from_config(config.try_into()?));

        let market_stream = {
            let mut stream = create_stream(config, vec![SubscribeTarget::Account])?;
            MarketService::new().bind_to(&mut stream);
            Arc::new(Mutex::new(stream))
        };

        Ok(Self {
            market_stream,
            mint_service: Arc::new(MintService::new(rpc_client)),
        })
    }
}

fn create_stream(
    config: &Config,
    targets: Vec<SubscribeTarget>,
) -> anyhow::Result<Box<dyn SolanaStream>> {
    match config.transport {
        Transport::Websocket => {
            let mut cfg: StreamConfig = config.try_into()?;
            cfg.targets = targets;
            Ok(Box::new(StreamClient::from_config(cfg)))
        }
        Transport::Grpc => {
            let mut cfg: GrpcConfig = config.try_into()?;
            cfg.targets = targets;
            Ok(Box::new(GrpcClient::from_config(cfg)))
        }
    }
}
