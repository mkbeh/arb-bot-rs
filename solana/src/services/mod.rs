use std::sync::Arc;

use async_trait::async_trait;
use engine::{Exchange, Sender, ServiceFactory};

use crate::{
    Config,
    services::{exchange::service::ExchangeService, sender::service::SenderService},
};

pub mod exchange;
pub mod sender;

pub struct Provider;

#[async_trait]
impl ServiceFactory<dyn Exchange> for Provider {
    type Config = Config;

    async fn from_config(config: &Config) -> anyhow::Result<Arc<dyn Exchange>> {
        Ok(Arc::new(ExchangeService::from_config(config).await?))
    }
}

#[async_trait]
impl ServiceFactory<dyn Sender> for Provider {
    type Config = Config;

    async fn from_config(config: &Config) -> anyhow::Result<Arc<dyn Sender>> {
        Ok(Arc::new(SenderService::from_config(config).await?))
    }
}
