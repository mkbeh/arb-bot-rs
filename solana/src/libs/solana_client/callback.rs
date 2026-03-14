use std::pin::Pin;

use crate::libs::solana_client::models::Event;

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub trait BatchEventHandler: Send + Sync + 'static {
    fn call(&mut self, events: Vec<Event>) -> BoxFuture<'static, anyhow::Result<()>>;
}

impl<F, Fut> BatchEventHandler for F
where
    F: FnMut(Vec<Event>) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = anyhow::Result<()>> + Send + 'static,
{
    fn call(&mut self, events: Vec<Event>) -> BoxFuture<'static, anyhow::Result<()>> {
        Box::pin((self)(events))
    }
}

type BatchEventCallback = Box<dyn BatchEventHandler>;

pub struct BatchEventCallbackWrapper {
    inner: BatchEventCallback,
}

impl BatchEventCallbackWrapper {
    pub fn new<F: BatchEventHandler>(callback: F) -> Self {
        Self {
            inner: Box::new(callback),
        }
    }

    pub async fn call(&mut self, events: Vec<Event>) -> anyhow::Result<()> {
        if events.is_empty() {
            return Ok(());
        }
        self.inner.call(events).await
    }
}
