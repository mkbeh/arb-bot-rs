use std::sync::Arc;

use async_trait::async_trait;

#[async_trait]
pub trait ServiceFactory<T: ?Sized> {
    type Config;
    async fn from_config(config: &Self::Config) -> anyhow::Result<Arc<T>>;
}
