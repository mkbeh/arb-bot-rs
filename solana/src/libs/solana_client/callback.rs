use std::sync::Arc;

use tokio::sync::Mutex;

use crate::libs::solana_client::dex::model::Event;

type BatchEventCallback = Box<dyn FnMut(Vec<Event>) -> anyhow::Result<()> + Send + 'static>;

#[derive(Clone)]
pub struct BatchEventCallbackWrapper {
    inner: Arc<Mutex<BatchEventCallback>>,
}

impl BatchEventCallbackWrapper {
    pub fn new<F>(callback: F) -> Self
    where
        F: FnMut(Vec<Event>) -> anyhow::Result<()> + Send + 'static,
    {
        Self {
            inner: Arc::new(Mutex::new(Box::new(callback))),
        }
    }

    /// Invokes the callback with the given event.
    pub async fn call(&self, events: Vec<Event>) -> anyhow::Result<()> {
        if events.is_empty() {
            return Ok(());
        }

        let mut guard = self.inner.lock().await;
        guard(events)
    }
}
