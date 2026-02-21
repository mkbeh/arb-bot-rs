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
        GrpcClient, GrpcConfig, SolanaStream, StreamClient, StreamConfig, models::SubscribeTarget,
    },
    services::exchange::{cache, market::MarketService},
};

pub struct ExchangeService {
    market_stream: Arc<Mutex<Box<dyn SolanaStream>>>,
}

impl Exchange for ExchangeService {}

#[async_trait]
impl ArbitrageService for ExchangeService {
    async fn start(&self, token: CancellationToken) -> anyhow::Result<()> {
        let mut tasks_set = JoinSet::new();

        tasks_set.spawn({
            let token = token.clone();
            let stream = self.market_stream.clone();

            async move {
                let mut stream = stream.lock().await;
                stream.subscribe(token).await
            }
        });

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
        let mut market_stream = create_stream(config, vec![SubscribeTarget::Account])?;

        let market = MarketService::new(config.liquidity_depth);
        market.bind_to(&mut market_stream);

        cache::init_metrics();

        Ok(Self {
            market_stream: Arc::new(Mutex::new(market_stream)),
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
