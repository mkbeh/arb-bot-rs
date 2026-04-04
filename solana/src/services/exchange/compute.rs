use std::hash::{Hash, Hasher};

use ahash::{AHashMap, AHashSet};
use metrics::{Unit, describe_gauge, gauge};
use rayon::prelude::*;
use solana_sdk::pubkey::Pubkey;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::error;

use crate::services::exchange::cache::{MarketState, PoolCache, TokenPair, get_market_state};

pub struct PoolUpdate {
    pub changed_pools: Vec<(Pubkey, u64)>,
    pub new_pools: Vec<Pubkey>,
}

pub struct ComputeService {
    path_manager: PathManager,
    base_mints: AHashSet<Pubkey>,
    tx: mpsc::Sender<PoolUpdate>,
    rx: mpsc::Receiver<PoolUpdate>,
}

impl ComputeService {
    #[must_use]
    pub fn new(base_mints: AHashSet<Pubkey>) -> Self {
        let (tx, rx) = mpsc::channel(1024);
        Self {
            path_manager: PathManager::new(),
            base_mints,
            tx,
            rx,
        }
    }

    #[must_use]
    pub fn sender(&self) -> mpsc::Sender<PoolUpdate> {
        self.tx.clone()
    }

    pub async fn start(&mut self, token: CancellationToken) -> anyhow::Result<()> {
        loop {
            tokio::select! {
                _ = token.cancelled() => {
                    return Ok(());
                }
                Some(update) = self.rx.recv() => {
                    self.handle_update(update).await?;
                }
            }
        }
    }

    async fn handle_update(&mut self, update: PoolUpdate) -> anyhow::Result<()> {
        let market = get_market_state().read();

        if !update.new_pools.is_empty() {
            self.path_manager
                .add_pools(&update.new_pools, &self.base_mints, market.pools());
        }

        let paths: Vec<&ComputePath> = self
            .path_manager
            .get_paths_for_pools(&update.changed_pools)
            .collect();

        if paths.is_empty() {
            return Ok(());
        }

        paths.par_iter().for_each(|path| {
            if let Err(e) = self.evaluate_path(path, &market) {
                error!("Calculation error: {}", e);
            }
        });

        Ok(())
    }

    #[allow(clippy::unused_self)]
    fn evaluate_path(&self, _path: &ComputePath, _market: &MarketState) -> anyhow::Result<()> {
        Ok(())
    }
}

/// A single swap step within an arbitrage path.
#[derive(Hash)]
struct ComputeStep {
    /// The pool used for this swap.
    pub pool_id: Pubkey,
    /// The token being sold.
    pub mint_in: Pubkey,
    /// The token being bought.
    pub mint_out: Pubkey,
    /// True if token_in is mint_a of the pool, false if token_in is mint_b.
    pub a_to_b: bool,
}

/// A full arbitrage cycle — a sequence of swaps that starts and ends
/// with the same base token.
struct ComputePath {
    /// The token the cycle starts and ends with (must be a base asset).
    pub base_token: Pubkey,
    /// Two swap steps forming the cycle.
    pub steps: [ComputeStep; 2],
}

impl Hash for ComputePath {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.base_token.hash(state);
        for step in &self.steps {
            step.hash(state);
        }
    }
}

/// Stores pre-computed arbitrage paths indexed by pool ID.
struct PathManager {
    /// Single source of truth: path_hash → ArbPath.
    paths: AHashMap<u64, ComputePath>,
    /// Index: pool_id → set of path hashes passing through it.
    index: AHashMap<Pubkey, Vec<u64>>,
    /// Hash factory.
    hash_builder: ahash::RandomState,
}

impl PathManager {
    const METRIC_PATHS_TOTAL: &str = "compute_paths_total";
    const METRIC_POOLS_INDEXED_TOTAL: &str = "compute_pools_indexed_total";

    #[must_use]
    pub fn new() -> Self {
        describe_gauge!(
            Self::METRIC_PATHS_TOTAL,
            Unit::Count,
            "Total number of unique arbitrage paths"
        );
        describe_gauge!(
            Self::METRIC_POOLS_INDEXED_TOTAL,
            Unit::Count,
            "Total number of unique pools in the path index"
        );

        Self {
            paths: AHashMap::new(),
            index: AHashMap::new(),
            hash_builder: ahash::RandomState::new(),
        }
    }

    /// Called when new pools appear in cache.
    /// Finds all 2-step arb paths through new pools that involve base assets.
    pub fn add_pools(
        &mut self,
        pool_ids: &[Pubkey],
        base_assets: &AHashSet<Pubkey>,
        pool_cache: &PoolCache,
    ) {
        for &pool_id in pool_ids {
            let Some(pool) = pool_cache.get_pool(&pool_id) else {
                continue;
            };

            let (mint_a, mint_b) = pool.get_mints();
            let pair = TokenPair::new(mint_a, mint_b);

            let base_mints: Vec<Pubkey> = [mint_a, mint_b]
                .into_iter()
                .filter(|m| base_assets.contains(m))
                .collect();

            if base_mints.is_empty() {
                continue;
            }

            let Some(sibling_ids) = pool_cache.get_pair_pool_ids(&pair) else {
                continue;
            };

            for &sibling_id in sibling_ids.iter().filter(|&&id| id != pool_id) {
                let Some(sibling) = pool_cache.get_pool(&sibling_id) else {
                    continue;
                };

                let (sibling_mint_a, _) = sibling.get_mints();

                for &base_mint in &base_mints {
                    let quote_mint = if base_mint == mint_a { mint_b } else { mint_a };

                    // base → quote via pool_id, quote → base via sibling
                    self.insert(
                        &[pool_id, sibling_id],
                        ComputePath {
                            base_token: base_mint,
                            steps: [
                                ComputeStep {
                                    pool_id,
                                    mint_in: base_mint,
                                    mint_out: quote_mint,
                                    a_to_b: base_mint == mint_a,
                                },
                                ComputeStep {
                                    pool_id: sibling_id,
                                    mint_in: quote_mint,
                                    mint_out: base_mint,
                                    a_to_b: quote_mint == sibling_mint_a,
                                },
                            ],
                        },
                    );

                    // base → quote via sibling, quote → base via pool_id
                    self.insert(
                        &[pool_id, sibling_id],
                        ComputePath {
                            base_token: base_mint,
                            steps: [
                                ComputeStep {
                                    pool_id: sibling_id,
                                    mint_in: base_mint,
                                    mint_out: quote_mint,
                                    a_to_b: base_mint == sibling_mint_a,
                                },
                                ComputeStep {
                                    pool_id,
                                    mint_in: quote_mint,
                                    mint_out: base_mint,
                                    a_to_b: quote_mint == mint_a,
                                },
                            ],
                        },
                    )
                }
            }
        }
        self.record_metrics()
    }

    /// Returns all arbitrage paths that pass through the given pools.
    pub fn get_paths_for_pools<'a>(
        &'a self,
        pool_ids: &'a [(Pubkey, u64)],
    ) -> impl Iterator<Item = &'a ComputePath> {
        let mut seen = AHashSet::new();
        pool_ids
            .iter()
            .flat_map(|(pool_id, _)| self.index.get(pool_id).into_iter().flat_map(|h| h.iter()))
            .filter(move |&&hash| seen.insert(hash))
            .filter_map(|hash| self.paths.get(hash))
    }

    /// Inserts a path into the store and registers it under all given pool IDs.
    fn insert(&mut self, pool_ids: &[Pubkey], path: ComputePath) {
        let hash = self.compute_hash(&path);
        self.paths.entry(hash).or_insert(path);
        for &pool_id in pool_ids {
            let entries = self.index.entry(pool_id).or_default();
            if !entries.contains(&hash) {
                entries.push(hash);
            }
        }
    }

    fn compute_hash(&self, path: &ComputePath) -> u64 {
        self.hash_builder.hash_one(path)
    }

    fn record_metrics(&self) {
        gauge!(Self::METRIC_PATHS_TOTAL).set(self.paths.len() as f64);
        gauge!(Self::METRIC_POOLS_INDEXED_TOTAL).set(self.index.len() as f64);
    }
}

impl Default for PathManager {
    fn default() -> Self {
        Self::new()
    }
}
