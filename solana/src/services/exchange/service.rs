use std::time;

use async_trait::async_trait;
use engine::{Exchange, service::traits::ArbitrageService};
use tokio_util::sync::CancellationToken;

use crate::{
    Config,
    libs::solana_client::{
        Stream, StreamConfig,
        models::{Event, SubscribeTarget},
    },
};

/// Core service for exchange arbitrage operations.
pub struct ExchangeService {}

impl Exchange for ExchangeService {}

#[async_trait]
impl ArbitrageService for ExchangeService {
    async fn start(&self, _token: CancellationToken) -> anyhow::Result<()> {
        let cfg = StreamConfig {
            endpoint: "wss://mainnet.helius-rpc.com/?api-key=42dce927-a807-4bfd-b125-844c27156ee4"
                .to_string(),
            ping_interval: time::Duration::from_secs(30),
            batch_size: 128,
            batch_fill_timeout: time::Duration::from_micros(10),
            program_ids: vec![
                // "LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo".to_string(),
                // "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8".to_string(),
                // "CPMMoo8L3F4NbTegBCKVNunggL7H1ZpdTHKxQB5qKP1C".to_string(),
                // "CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK".to_string(),
                // "cpamdpZCGKUy5JxQXB4dcpGPiikHawvSWAd6mEn1sGG".to_string(),
                // "whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc".to_string(),
                // "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P".to_string(),
            ],
            targets: vec![
                // SubscribeTarget::Slot,
                SubscribeTarget::Account,
                // SubscribeTarget::Transaction,
            ],
        };

        Stream::from_config(cfg)
            .with_callback(|events| {
                for event in events {
                    match event {
                        Event::BlockMeta(_) => {}
                        Event::Slot(slot) => {
                            // println!("slot: {:?}", slot)
                        }
                        Event::Tx(tx) => {
                            // println!("tx: {:?}", tx);
                        }
                        Event::Account(acc) => {
                            println!("acc: {:?}", acc);
                        }
                    }
                }

                Ok(())
            })
            .subscribe(_token.clone())
            .await?;

        Ok(())
    }
}

impl ExchangeService {
    pub async fn from_config(_config: &Config) -> anyhow::Result<Self> {
        Ok(Self {})
    }
}
