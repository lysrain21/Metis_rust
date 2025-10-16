use std::collections::{HashMap, HashSet};

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
