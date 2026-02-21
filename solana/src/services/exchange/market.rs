use std::sync::Arc;

use parking_lot::RwLock;

use crate::{
    libs::solana_client::{
        SolanaStream,
        callback::{BatchEventCallbackWrapper, BatchEventHandler},
        models::Event,
    },
    services::exchange::cache::MarketState,
};

pub struct MarketService {
    state: Arc<RwLock<MarketState>>,
}

impl MarketService {
    #[must_use]
    pub fn new(liquidity_depth: i64) -> Self {
        Self {
            state: Arc::new(RwLock::new(MarketState::new(liquidity_depth))),
        }
    }

    pub fn bind_to(&self, stream: &mut Box<dyn SolanaStream>) {
        let wrapper = BatchEventCallbackWrapper::new(self.handle_events(self.state.clone()));
        stream.set_callback(wrapper)
    }

    fn handle_events(&self, state: Arc<RwLock<MarketState>>) -> impl BatchEventHandler {
        move |events: Vec<Event>| {
            let mut market = state.write();

            for event in events {
                let Event::Account(acc) = event else {
                    continue;
                };

                let Some(target_pk) = market.update_state(acc.pubkey, acc.pool_state) else {
                    continue;
                };

                // todo: arbitrage logic
            }

            Ok(())
        }
    }
}
