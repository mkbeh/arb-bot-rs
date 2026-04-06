use std::hash::{Hash, Hasher};

use ahash::{AHashMap, AHashSet};
use metrics::{Unit, describe_gauge, gauge};
use rayon::prelude::*;
use solana_sdk::{account::Account, pubkey::Pubkey};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::error;

use crate::{libs::solana_client::pool::*, services::exchange::cache::*};

/// Basis points denominator (10_000 bps = 100%).
pub const BPS_DENOMINATOR: u64 = 10_000;

pub struct PoolUpdate {
    pub changed_pools: Vec<(Pubkey, u64)>,
    pub new_pools: Vec<Pubkey>,
}

/// Configuration for the compute service.
pub struct ComputeConfig {
    /// Base token mints to use as entry/exit points for arbitrage cycles.
    pub base_mints: AHashSet<Pubkey>,
    /// Min fraction of Kamino reserve to use as input in bps (e.g. 10 = 0.1%).
    /// Also used as the ternary search precision threshold.
    pub min_liquidity_fraction_bps: u64,
    /// Max fraction of Kamino reserve to use as input in bps (e.g. 2000 = 20%).
    pub max_liquidity_fraction_bps: u64,
    /// Minimum profit as a fraction of amount_in in bps (e.g. 10 = 0.1%).
    pub min_profit_bps: u64,
}

/// A detected arbitrage opportunity ready for execution.
#[derive(Debug)]
pub struct ArbOpportunity {
    /// The path that generated this opportunity.
    pub path: ComputePath,
    /// Optimal input amount in base token native units.
    pub amount_in: u64,
    /// Output amount in base token native units after both swaps.
    pub amount_out: u64,
    /// Gross profit in base token native units.
    pub profit: u64,
    /// Quote results for each step.
    pub step_quotes: [QuoteResult; 2],
}

/// Receives pool updates, maintains pre-computed arbitrage paths,
/// and evaluates them in parallel to detect profitable opportunities.
pub struct ComputeService {
    /// Pre-computed arbitrage paths indexed by pool ID.
    path_manager: PathManager,
    /// Runtime configuration.
    config: ComputeConfig,
    tx: mpsc::Sender<PoolUpdate>,
    rx: mpsc::Receiver<PoolUpdate>,
}

impl ComputeService {
    #[must_use]
    pub fn new(config: ComputeConfig) -> Self {
        let (tx, rx) = mpsc::channel(1024);
        Self {
            path_manager: PathManager::new(),
            config,
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

    /// Processes a single pool update batch:
    /// 1. Adds newly discovered pools to the path graph.
    /// 2. Evaluates all affected arbitrage paths in parallel.
    async fn handle_update(&mut self, update: PoolUpdate) -> anyhow::Result<()> {
        let market = get_market_state().read();

        if !update.new_pools.is_empty() {
            self.path_manager
                .add_pools(&update.new_pools, &self.config.base_mints, market.pools());
        }

        let paths: Vec<&ComputePath> = self
            .path_manager
            .get_paths_for_pools(&update.changed_pools)
            .collect();

        if paths.is_empty() {
            return Ok(());
        }

        let mint_cache = get_mint_cache().read();
        let amm_config_cache = get_amm_config_cache().read();

        paths.par_iter().for_each(|path| {
            match self.evaluate_path(path, &market, &mint_cache, &amm_config_cache) {
                Ok(Some(_opportunity)) => {
                    // todo: send opportunity to executor
                }
                Ok(None) => {}
                Err(e) => error!("evaluate_path error: {e}"),
            }
        });

        Ok(())
    }

    fn evaluate_path(
        &self,
        path: &ComputePath,
        market: &MarketState,
        mint_cache: &MintCache,
        amm_config_cache: &AmmConfigCache,
    ) -> anyhow::Result<Option<ArbOpportunity>> {
        let step0 = &path.steps[0];
        let step1 = &path.steps[1];

        let mint_in0 = mint_cache
            .get(&step0.mint_in)
            .ok_or_else(|| anyhow::anyhow!("Mint not found: {}", step0.mint_in))?;
        let mint_out0 = mint_cache
            .get(&step0.mint_out)
            .ok_or_else(|| anyhow::anyhow!("Mint not found: {}", step0.mint_out))?;
        let mint_out1 = mint_cache
            .get(&step1.mint_out)
            .ok_or_else(|| anyhow::anyhow!("Mint not found: {}", step1.mint_out))?;

        let pool0 = market
            .pools()
            .get_pool(&step0.pool_id)
            .ok_or_else(|| anyhow::anyhow!("Pool not found: {}", step0.pool_id))?;
        let pool1 = market
            .pools()
            .get_pool(&step1.pool_id)
            .ok_or_else(|| anyhow::anyhow!("Pool not found: {}", step1.pool_id))?;

        let reserve = market
            .reserves()
            .get(&path.base_token)
            .ok_or_else(|| anyhow::anyhow!("No reserve found for mint: {}", path.base_token))?;

        let min_amount = reserve
            .total_available_amount
            .saturating_mul(self.config.min_liquidity_fraction_bps)
            / BPS_DENOMINATOR;

        let max_amount = reserve
            .total_available_amount
            .saturating_mul(self.config.max_liquidity_fraction_bps)
            / BPS_DENOMINATOR;

        if min_amount >= max_amount {
            return Ok(None);
        }

        let Some((amount_in, profit, quote0, quote1)) = self.find_best_opportunity(
            path,
            market,
            min_amount,
            max_amount,
            mint_in0,
            mint_out0,
            mint_out1,
            pool0,
            pool1,
            amm_config_cache,
        ) else {
            return Ok(None);
        };

        Ok(Some(ArbOpportunity {
            path: path.clone(),
            amount_in,
            amount_out: quote1.total_amount_out,
            profit,
            step_quotes: [quote0, quote1],
        }))
    }

    /// Runs a ternary search over `[min_amount, max_amount]` to find the input amount
    /// that maximises profit for the given path.
    ///
    /// The search terminates when the interval narrows below `min_amount` (precision threshold).
    /// Returns the best `(amount_in, profit, quote0, quote1)` found, or `None` if no
    /// profitable opportunity exceeds `min_profit_bps`.
    #[allow(clippy::too_many_arguments)]
    fn find_best_opportunity(
        &self,
        path: &ComputePath,
        market: &MarketState,
        min_amount: u64,
        max_amount: u64,
        mint_in0: &Account,
        mint_out0: &Account,
        mint_out1: &Account,
        pool0: &dyn DexPool,
        pool1: &dyn DexPool,
        amm_config_cache: &AmmConfigCache,
    ) -> Option<(u64, u64, QuoteResult, QuoteResult)> {
        let precision = min_amount;

        let mut lo = min_amount;
        let mut hi = max_amount;
        let mut best_result: Option<(u64, u64, QuoteResult, QuoteResult)> = None;
        let mut max_profit = 0u64;

        loop {
            if hi.saturating_sub(lo) < precision {
                break;
            }

            let m1 = lo + (hi - lo) / 3;
            let m2 = hi - (hi - lo) / 3;

            let res1 = Self::compute_profit(
                path,
                market,
                m1,
                mint_in0,
                mint_out0,
                mint_out1,
                pool0,
                pool1,
                amm_config_cache,
            );
            let res2 = Self::compute_profit(
                path,
                market,
                m2,
                mint_in0,
                mint_out0,
                mint_out1,
                pool0,
                pool1,
                amm_config_cache,
            );

            let p1 = res1.as_ref().map(|(p, _, _)| *p).unwrap_or(0);
            let p2 = res2.as_ref().map(|(p, _, _)| *p).unwrap_or(0);

            if p1 > max_profit {
                max_profit = p1;
                best_result = res1.map(|(p, q0, q1)| (m1, p, q0, q1));
            }
            if p2 > max_profit {
                max_profit = p2;
                best_result = res2.map(|(p, q0, q1)| (m2, p, q0, q1));
            }

            if p1 < p2 {
                lo = m1;
            } else {
                hi = m2;
            }
        }

        best_result.filter(|(amount_in, profit, _, _)| {
            let min_profit = amount_in.saturating_mul(self.config.min_profit_bps) / BPS_DENOMINATOR;
            *profit >= min_profit
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn compute_profit(
        path: &ComputePath,
        market: &MarketState,
        amount_in: u64,
        mint_in0: &Account,
        mint_out0: &Account,
        mint_out1: &Account,
        pool0: &dyn DexPool,
        pool1: &dyn DexPool,
        amm_config_cache: &AmmConfigCache,
    ) -> Option<(u64, QuoteResult, QuoteResult)> {
        let step0 = &path.steps[0];
        let step1 = &path.steps[1];

        let clock = market.clock()?;

        let quote0 = pool0
            .quote(&QuoteContext {
                quote_type: QuoteType::ExactIn(amount_in),
                a_to_b: step0.a_to_b,
                clock,
                mint_in: mint_in0,
                mint_out: mint_out0,
                vaults: pool0
                    .get_vault_pubkeys()
                    .and_then(|(a, b)| market.vaults().get_pair(&a, &b)),
                liquidity: market.liquidity().get_map(&step0.pool_id),
                bitmap: market.bitmaps().get(&step0.pool_id),
                amm_config: pool0
                    .get_amm_config_pubkey()
                    .and_then(|key| amm_config_cache.get(&key)),
                oracle: market.oracles().get(&step0.pool_id),
            })
            .ok()?;

        if quote0.total_amount_out == 0 {
            return None;
        }

        let quote1 = pool1
            .quote(&QuoteContext {
                quote_type: QuoteType::ExactIn(quote0.total_amount_out),
                a_to_b: step1.a_to_b,
                clock,
                mint_in: mint_out0,
                mint_out: mint_out1,
                vaults: pool1
                    .get_vault_pubkeys()
                    .and_then(|(a, b)| market.vaults().get_pair(&a, &b)),
                liquidity: market.liquidity().get_map(&step1.pool_id),
                bitmap: market.bitmaps().get(&step1.pool_id),
                amm_config: pool1
                    .get_amm_config_pubkey()
                    .and_then(|key| amm_config_cache.get(&key)),
                oracle: market.oracles().get(&step1.pool_id),
            })
            .ok()?;

        let profit = quote1.total_amount_out.checked_sub(amount_in)?;
        Some((profit, quote0, quote1))
    }
}

/// A single swap step within an arbitrage path.
#[derive(Debug, Clone, Hash)]
pub struct ComputeStep {
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
#[derive(Debug, Clone)]
pub struct ComputePath {
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
    /// Single source of truth: path_hash → ComputePath.
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
