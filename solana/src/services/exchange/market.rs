use ahash::AHashSet;

use crate::{
    libs::solana_client::{
        SolanaStream,
        callback::{BatchEventCallbackWrapper, BatchEventHandler},
        models::Event,
    },
    services::exchange::cache::get_market_state,
};

/// Processes incoming on-chain account events and updates the global market state.
pub struct MarketService;

impl Default for MarketService {
    fn default() -> Self {
        Self::new()
    }
}

impl MarketService {
    /// Creates a new `MarketService` instance.
    #[must_use]
    pub fn new() -> Self {
        Self {}
    }

    /// Attaches the market event handler to the given stream.
    pub fn bind_to(&self, stream: &mut Box<dyn SolanaStream>) {
        let wrapper = BatchEventCallbackWrapper::new(Self::handle_events());
        stream.set_callback(wrapper)
    }

    /// Returns a batch event handler that processes account updates.
    fn handle_events() -> impl BatchEventHandler {
        move |events: Vec<Event>| {
            let mut changed_pools = AHashSet::with_capacity(events.len());

            {
                let mut market = get_market_state().write();
                for event in events {
                    let Event::Account(acc) = event else {
                        continue;
                    };

                    let Some(pool_id) = market.update_state(acc.pubkey, acc.pool_state) else {
                        continue;
                    };

                    changed_pools.insert(pool_id);
                }
            }

            // todo: other logic

            Ok(())
        }
    }
}
