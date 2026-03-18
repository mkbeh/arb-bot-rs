use std::sync::Arc;

use anyhow::anyhow;
use async_trait::async_trait;
use engine::{Exchange, service::traits::ArbitrageService};
use tokio::{sync::Mutex, task::JoinSet};
use tokio_util::sync::CancellationToken;

use crate::{
    Config,
    config::Transport,
    libs::solana_client::*,
    services::exchange::{
        background::{AmmConfigService, BackgroundService, MintService},
        cache,
        compute::ComputeService,
        market::MarketService,
    },
};

pub struct ExchangeService {
    market_stream: Arc<Mutex<Box<dyn SolanaStream>>>,
    compute_service: Arc<ComputeService>,
    background_services: Vec<Arc<dyn BackgroundService + Send + Sync>>,
}

impl Exchange for ExchangeService {}

#[async_trait]
impl ArbitrageService for ExchangeService {
    async fn start(&self, token: CancellationToken) -> anyhow::Result<()> {
        let mut tasks_set = JoinSet::new();

        // Background jobs.
        for service in &self.background_services {
            tasks_set.spawn({
                let token = token.clone();
                let service = service.clone();
                async move { service.start(token).await }
            });
        }

        // Main market stream: subscribes to on-chain account updates via websocket/gRPC
        tasks_set.spawn({
            let token = token.clone();
            let stream = self.market_stream.clone();

            async move {
                let mut stream = stream.lock().await;
                stream.subscribe(token).await
            }
        });

        // Spawn the compute service task responsible for detecting
        // and evaluating arbitrage opportunities from pool updates.
        tasks_set.spawn({
            let token = token.clone();
            let compute = self.compute_service.clone();
            async move { compute.start(token).await }
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

        let rpc = Arc::new(RpcClient::from_config(config.try_into()?));
        let compute_service = ComputeService::new();

        let mut stream = create_stream(config, vec![SubscribeTarget::Account])?;
        Arc::new(MarketService::new(rpc.clone(), compute_service.sender())).bind_to(&mut stream);

        Ok(Self {
            market_stream: Arc::new(Mutex::new(stream)),
            compute_service: Arc::new(compute_service),
            background_services: vec![
                Arc::new(MintService::new(rpc.clone())),
                Arc::new(AmmConfigService::new(rpc)),
            ],
        })
    }
}

fn create_stream(
    config: &Config,
    targets: Vec<SubscribeTarget>,
) -> anyhow::Result<Box<dyn SolanaStream>> {
    match config.transport {
        Transport::Websocket => {
            let mut cfg: WebsocketStreamConfig = config.try_into()?;
            cfg.targets = targets;
            Ok(Box::new(WebsocketStream::from_config(cfg)))
        }
        Transport::Grpc => {
            let mut cfg: GrpcStreamConfig = config.try_into()?;
            cfg.targets = targets;
            Ok(Box::new(GrpcStream::from_config(cfg)))
        }
    }
}
