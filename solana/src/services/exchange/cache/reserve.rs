use ahash::AHashMap;
use solana_sdk::pubkey::Pubkey;

use crate::{
    libs::solana_client::{protocols::kamino::*, utils},
    services::exchange::cache::RESERVE_CACHE_METRICS,
};

#[derive(Debug, Clone)]
pub struct ReserveData {
    /// Total available liquidity in the reserve.
    pub total_available_amount: u64,
    /// Flash loan fee in scaled fraction format (U68F60 bits).
    pub flash_loan_fee_sf: u64,
    /// Timestamp of the last update in milliseconds.
    pub updated_at: u64,
}

/// Cache of Kamino lending reserves, keyed by token mint address.
pub struct ReserveCache {
    data: AHashMap<Pubkey, ReserveData>,
}

impl Default for ReserveCache {
    fn default() -> Self {
        Self::new()
    }
}

impl ReserveCache {
    #[must_use]
    pub fn new() -> Self {
        Self {
            data: AHashMap::with_capacity(32),
        }
    }

    pub fn update(&mut self, reserve: &Reserve) {
        let mint = Pubkey::from(reserve.liquidity.mint_pubkey);
        let updated_at = utils::get_timestamp_ms();
        let amount = reserve.liquidity.total_available_amount as f64
            / 10f64.powi(reserve.liquidity.mint_decimals as i32);

        let prev = self.data.insert(
            mint,
            ReserveData {
                total_available_amount: reserve.liquidity.total_available_amount,
                flash_loan_fee_sf: reserve.config.fees.flash_loan_fee_sf,
                updated_at,
            },
        );

        if prev.is_none() {
            RESERVE_CACHE_METRICS.record(&mint, amount, updated_at);
        }
    }

    #[inline]
    #[must_use]
    pub fn get(&self, pubkey: &Pubkey) -> Option<&ReserveData> {
        self.data.get(pubkey)
    }

    #[must_use]
    pub fn calculate_repay_amount(&self, pubkey: &Pubkey, borrow_amount: u64) -> Option<u64> {
        let reserve = self.get(pubkey)?;

        if reserve.total_available_amount < borrow_amount {
            return None;
        }

        Some(calculate_flash_loan_repay_amount(
            reserve.flash_loan_fee_sf,
            borrow_amount,
        ))
    }
}
