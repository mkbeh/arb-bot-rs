use solana_sdk::pubkey::Pubkey;

use super::quote::{QuoteContext, QuoteResult};
use crate::libs::solana_client::metrics::DexMetrics;

/// Core trait for DEX pool implementations.
///
/// Any pool that supports swap simulation must implement this trait.
/// It provides access to the pool's token mints and the ability to
/// simulate swaps via [`quote`](DexPool::quote).
pub trait DexPool: DexMetrics + Send + Sync {
    /// Returns the mint address of token A (input token for a→b swaps).
    fn get_mint_a(&self) -> Pubkey;

    /// Returns the mint address of token B (output token for a→b swaps).
    fn get_mint_b(&self) -> Pubkey;

    /// Returns both mint addresses as a tuple `(mint_a, mint_b)`.
    fn get_mints(&self) -> (Pubkey, Pubkey) {
        (self.get_mint_a(), self.get_mint_b())
    }

    /// Returns vault pubkeys (token_a_vault, token_b_vault) if pool uses external vaults.
    fn get_vault_pubkeys(&self) -> Option<(Pubkey, Pubkey)> {
        None
    }

    /// Simulates a swap and returns a detailed quote.
    ///
    /// # Arguments
    /// * `ctx` — swap parameters
    /// * `data` — protocol-specific liquidity arrays from cache
    ///
    /// # Errors
    /// Returns an error if the pool state is invalid or liquidity is insufficient.
    fn quote(&self, ctx: &QuoteContext) -> anyhow::Result<QuoteResult>;
}
