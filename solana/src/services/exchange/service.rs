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
    services::exchange::{
        background::{AmmConfigService, BackgroundService, MintService},
        cache,
        compute::ComputeService,
        market::MarketService,
    },
};

pub struct ExchangeService {
    market_stream: Arc<Mutex<Box<dyn SolanaStream>>>,
    compute_service: Arc<Mutex<ComputeService>>,
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
            async move {
                let mut compute = compute.lock().await;
                compute.start(token).await
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

        let rpc = Arc::new(RpcClient::from_config(config.try_into()?));
        let compute_service = ComputeService::new(config.get_mints_addrs());

        let mut market_stream = build_market_stream(config)?;
        Arc::new(MarketService::new(rpc.clone(), compute_service.sender()))
            .bind_to(&mut market_stream);

        Ok(Self {
            market_stream: Arc::new(Mutex::new(market_stream)),
            compute_service: Arc::new(Mutex::new(compute_service)),
            background_services: vec![
                Arc::new(MintService::new(rpc.clone())),
                Arc::new(AmmConfigService::new(rpc)),
            ],
        })
    }
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
    config
        .get_dex_addrs()
        .into_iter()
        .map(|addr| ProtocolConfig {
            program_id: addr,
            account_ids: vec![],
        })
        .chain(std::iter::once(ProtocolConfig {
            program_id: KAMINO_ID.to_string(),
            account_ids: config
                .get_reserves_addrs()
                .iter()
                .map(|p| p.to_string())
                .collect(),
        }))
        .collect()
}
