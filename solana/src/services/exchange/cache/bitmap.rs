use std::collections::hash_map::Entry;

use ahash::AHashMap;
use solana_sdk::pubkey::Pubkey;

use crate::{
    libs::solana_client::{
        metrics::*,
        pool::*,
        protocols::{meteora_dlmm::*, raydium_clmm::*},
    },
    services::exchange::cache::BITMAP_CACHE_METRICS,
};

pub enum CachedBitmap {
    MeteoraDlmm(Box<BinArrayBitmapExtension>),
    RaydiumClmm(Box<TickArrayBitmapExtension>),
}

impl CachedBitmap {
    #[must_use]
    pub fn protocol_name(&self) -> &'static str {
        match self {
            Self::MeteoraDlmm(_) => DEX_METEORA_DLMM,
            Self::RaydiumClmm(_) => DEX_RAYDIUM_CLMM,
        }
    }
}

pub struct BitmapCache {
    data: AHashMap<Pubkey, CachedBitmap>,
    slots: AHashMap<Pubkey, u64>,
}

impl Default for BitmapCache {
    fn default() -> Self {
        Self::new()
    }
}

impl BitmapCache {
    #[must_use]
    pub fn new() -> Self {
        Self {
            data: AHashMap::with_capacity(1024),
            slots: AHashMap::with_capacity(1024),
        }
    }

    pub fn update(&mut self, pubkey: Pubkey, slot: u64, bitmap: CachedBitmap) {
        let protocol_name = bitmap.protocol_name();
        match self.slots.entry(pubkey) {
            Entry::Occupied(mut occupied) => {
                if slot <= *occupied.get() {
                    return;
                }
                occupied.insert(slot);
            }
            Entry::Vacant(vacant) => {
                vacant.insert(slot);
            }
        }

        let prev = self.data.insert(pubkey, bitmap);
        if prev.is_none() {
            BITMAP_CACHE_METRICS.record(protocol_name);
        }
    }

    #[must_use]
    pub fn get(&self, pubkey: &Pubkey) -> Option<LiquidityBitmap<'_>> {
        self.data.get(pubkey).map(|b| match b {
            CachedBitmap::MeteoraDlmm(b) => LiquidityBitmap::MeteoraDlmm(Some(b)),
            CachedBitmap::RaydiumClmm(b) => LiquidityBitmap::RaydiumClmm(Some(b)),
        })
    }
}
