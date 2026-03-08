use std::sync::Arc;

use tokio::sync::Mutex;

use crate::libs::solana_client::models::Event;

pub trait BatchEventHandler: FnMut(Vec<Event>) -> anyhow::Result<()> + Send + 'static {}

impl<F> BatchEventHandler for F where F: FnMut(Vec<Event>) -> anyhow::Result<()> + Send + 'static {}

type BatchEventCallback = Box<dyn BatchEventHandler>;

#[derive(Clone)]
pub struct BatchEventCallbackWrapper {
    inner: Arc<Mutex<BatchEventCallback>>,
}

impl BatchEventCallbackWrapper {
    pub fn new<F: BatchEventHandler>(callback: F) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Box::new(callback))),
        }
    }

    /// Invokes the callback with the given events.
    pub async fn call(&self, events: Vec<Event>) -> anyhow::Result<()> {
        if events.is_empty() {
            return Ok(());
        }

        let mut guard = self.inner.lock().await;
        guard(events)
    }
}
