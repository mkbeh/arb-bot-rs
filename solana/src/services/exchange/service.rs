use std::sync::Arc;

use anyhow::anyhow;
use async_trait::async_trait;
use engine::{Exchange, service::traits::ArbitrageService};
use tokio::{sync::Mutex, task::JoinSet};
use tokio_util::sync::CancellationToken;

use crate::{
    Config,
    config::TransportConfig,
    libs::solana_client::{protocols::kamino::KAMINO_ID, *},
    services::exchange::{background::*, cache, compute::ComputeService, market::MarketService},
};

pub struct ExchangeService {
    market_stream: Mutex<Option<Box<dyn SolanaStream>>>,
    compute_service: Mutex<Option<ComputeService>>,
    background_services: Vec<Arc<dyn BackgroundService + Send + Sync>>,
}

impl Exchange for ExchangeService {}

#[async_trait]
impl ArbitrageService for ExchangeService {
    async fn start(&self, token: CancellationToken) -> anyhow::Result<()> {
        let mut tasks_set = JoinSet::new();

        let mut market_stream = self
            .market_stream
            .lock()
            .await
            .take()
            .ok_or_else(|| anyhow!("Market stream already started or not initialized"))?;

        let mut compute_service = self
            .compute_service
            .lock()
            .await
            .take()
            .ok_or_else(|| anyhow!("Compute service already started or not initialized"))?;

        // Background jobs.
        for service in &self.background_services {
            tasks_set.spawn({
                let token = token.clone();
                let service = service.clone();
                async move { service.start(token).await }
            });
        }

        // Main market stream: subscribes to on-chain account updates via websocket/gRPC.
        tasks_set.spawn({
            let token = token.clone();
            async move { market_stream.subscribe(token).await }
        });

        // Spawn the compute service task responsible for detecting
        // and evaluating arbitrage opportunities from pool updates.
        tasks_set.spawn({
            let token = token.clone();
            async move { compute_service.start(token).await }
        });

        // If any task finishes (either completes or errors),
        // cancel all others and propagate result.
        if let Some(result) = tasks_set.join_next().await {
            token.cancel();
            return result
                .map_err(|e| anyhow!("Task panicked: {e}"))
                .and_then(|r| r);
        }

        Ok(())
    }
}

impl ExchangeService {
    pub async fn from_config(config: &Config) -> anyhow::Result<Self> {
        cache::init(config.strategy.liquidity_depth)?;

        let rpc = Arc::new(RpcClient::from_config(config.try_into()?));
        let compute_service = ComputeService::new(config.try_into()?);

        let mut market_stream = build_market_stream(config)?;
        Arc::new(MarketService::new(rpc.clone(), compute_service.sender()))
            .bind_to(&mut market_stream);

        Ok(Self {
            market_stream: Mutex::new(Some(market_stream)),
            compute_service: Mutex::new(Some(compute_service)),
            background_services: build_background_services(rpc),
        })
    }
}

fn build_background_services(rpc: Arc<RpcClient>) -> Vec<Arc<dyn BackgroundService + Send + Sync>> {
    vec![
        Arc::new(MintService::new(rpc.clone())),
        Arc::new(AmmConfigService::new(rpc)),
    ]
}

fn build_market_stream(config: &Config) -> anyhow::Result<Box<dyn SolanaStream>> {
    let targets = vec![SubscribeTarget::Clock, SubscribeTarget::Program];
    let protocols = build_market_protocols(config);

    match config.transport {
        TransportConfig::Websocket { .. } => {
            let cfg = WebsocketStreamConfig {
                targets,
                protocols,
                ..config.try_into()?
            };
            Ok(Box::new(WebsocketStream::from_config(cfg)))
        }
        TransportConfig::Grpc { .. } => {
            let cfg = GrpcStreamConfig {
                targets,
                protocols,
                ..config.try_into()?
            };
            Ok(Box::new(GrpcStream::from_config(cfg)))
        }
    }
}

fn build_market_protocols(config: &Config) -> ProtocolMap {
    let mut protocols: Vec<ProtocolConfig> = config
        .get_dex_addrs()
        .into_iter()
        .map(|program_id| ProtocolConfig {
            program_id,
            account_ids: vec![],
        })
        .collect();

    protocols.push(ProtocolConfig {
        program_id: KAMINO_ID.to_string(),
        account_ids: config
            .get_reserves_addrs()
            .iter()
            .map(|p| p.to_string())
            .collect(),
    });

    protocols.into_iter().collect()
}
