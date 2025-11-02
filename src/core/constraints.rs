use std::collections::{HashMap, HashSet};

/// Split configuration controls allocation precision for optimization algorithms
#[derive(Debug, Clone)]
pub struct SplitConfig {
    pub min_precision_bps: u16,
    pub max_splits: usize,
    pub adaptive: bool,
}

impl SplitConfig {
    pub fn tolerance_amount(&self, total_amount: f64, routes: usize) -> f64 {
        let base = total_amount * (self.min_precision_bps.max(1) as f64 / 10_000.0);
        if self.adaptive && routes > 1 {
            (base / routes as f64).max(total_amount / (self.max_splits.max(1) as f64))
        } else {
            base.max(total_amount / (self.max_splits.max(1) as f64))
        }
    }

    pub fn max_iterations(&self) -> usize {
        self.max_splits.max(8)
    }
}

impl Default for SplitConfig {
    fn default() -> Self {
        Self {
            min_precision_bps: 5, // 0.05%
            max_splits: 128,
            adaptive: true,
        }
    }
}

/// Available optimization algorithms for capital allocation across routes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizationAlgorithm {
    Waterfill,
    GoldenSection,
    Brent,
    Hybrid,
}

/// Collection of tunables that shape candidate generation and allocations
#[derive(Debug, Clone)]
pub struct RoutingConstraints {
    pub max_hops: usize,
    pub max_paths: usize,
    pub candidate_pool_size: usize,
    pub intermediate_token_whitelist: Option<HashSet<String>>,
    pub slippage_tolerance_bps: f64,
    pub step_epsilon: Option<f64>,
    pub metadata: HashMap<String, String>,
    pub optimization_algorithm: OptimizationAlgorithm,
    pub split_config: Option<SplitConfig>,
}

impl Default for RoutingConstraints {
    fn default() -> Self {
        Self {
            max_hops: 5,
            max_paths: 4,
            candidate_pool_size: 12,
            intermediate_token_whitelist: None,
            slippage_tolerance_bps: 30.0,
            step_epsilon: None,
            metadata: HashMap::new(),
            optimization_algorithm: OptimizationAlgorithm::Waterfill,
            split_config: None,
        }
    }
}

impl RoutingConstraints {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_allowed_node(&self, node: &str, src: &str, dst: &str) -> bool {
        if node == src || node == dst {
            return true;
        }
        match &self.intermediate_token_whitelist {
            None => true,
            Some(whitelist) => whitelist.contains(node),
        }
    }
}
