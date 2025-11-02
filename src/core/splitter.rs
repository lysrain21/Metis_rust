use crate::core::constraints::{OptimizationAlgorithm, RoutingConstraints, SplitConfig};
use crate::core::graph::RoutingGraph;
use crate::core::optimization::{evaluate_allocation, run_brent, run_golden_section, run_hybrid};
use crate::core::plan::{RouteAlloc, RouteLeg, RoutePlan};
use crate::core::pool::Edge;
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet};

const MIN_STEP_SIZE: f64 = 1e-12;

pub fn generate_high_precision_splits(total_amount: f64, config: &SplitConfig) -> Vec<f64> {
    if total_amount <= 0.0 {
        return Vec::new();
    }

    let mut splits = Vec::new();
    let tolerance = config.tolerance_amount(total_amount, 1).max(MIN_STEP_SIZE);
    let max_points = config.max_splits.max(1);

    let mut current = tolerance;
    let mut count = 0;
    while current < total_amount && count + 1 < max_points {
        splits.push(current);
        current += tolerance;
        count += 1;
    }

    if splits
        .last()
        .map(|v| (total_amount - v).abs() > MIN_STEP_SIZE)
        .unwrap_or(true)
    {
        splits.push(total_amount);
    }

    splits
}

fn determine_step_size(
    amount_in: f64,
    constraints: &RoutingConstraints,
    candidate_len: usize,
) -> f64 {
    if let Some(config) = &constraints.split_config {
        let mut step = config.tolerance_amount(amount_in, candidate_len.max(1));
        if config.adaptive && candidate_len > 1 {
            let adaptive_cap = amount_in / (candidate_len.max(1) * 10) as f64;
            step = step.min(adaptive_cap.max(MIN_STEP_SIZE));
        }
        step.max(MIN_STEP_SIZE).min(amount_in)
    } else {
        (amount_in / 200.0).max(MIN_STEP_SIZE)
    }
}

fn algo_label(algo: OptimizationAlgorithm) -> &'static str {
    match algo {
        OptimizationAlgorithm::Waterfill => "waterfill",
        OptimizationAlgorithm::GoldenSection => "golden_section",
        OptimizationAlgorithm::Brent => "brent",
        OptimizationAlgorithm::Hybrid => "hybrid",
    }
}

fn apply_allocations_to_graph(
    rg: &mut RoutingGraph,
    candidates: &[Vec<Edge>],
    allocations: &[f64],
    constraints: &RoutingConstraints,
    algo_label: &str,
    extra_meta: Option<HashMap<String, String>>,
) -> RoutePlan {
    if allocations.is_empty() || candidates.is_empty() {
        let mut meta = HashMap::new();
        meta.insert("reason".to_string(), "no_candidates".to_string());
        meta.insert("algo".to_string(), algo_label.to_string());
        return RoutePlan::empty(Some(meta));
    }

    let slippage_factor = (1.0 - constraints.slippage_tolerance_bps / 10_000.0).max(0.0);
    let mut routes = Vec::new();

    for (idx, (path, &amount_in)) in candidates.iter().zip(allocations.iter()).enumerate() {
        if amount_in <= MIN_STEP_SIZE {
            continue;
        }
        if path.is_empty() {
            continue;
        }

        let mut current = amount_in;
        let mut legs = Vec::new();
        let mut valid = true;

        for edge in path {
            if let Some(pool) = rg.pools.get_mut(&edge.pool_id) {
                let pool_kind = pool.kind().to_string();
                let pool_meta = pool.meta().clone();
                let out = pool.apply_virtual_fill(current);
                legs.push(RouteLeg {
                    src: edge.src.clone(),
                    dst: edge.dst.clone(),
                    pool_id: edge.pool_id.clone(),
                    pool_kind,
                    edge_name: edge.name.clone(),
                    metadata: pool_meta,
                });
                current = out;
            } else {
                valid = false;
                break;
            }
        }

        if !valid || current <= 0.0 {
            continue;
        }

        let mut route_meta = HashMap::new();
        let path_names: Vec<String> = path.iter().map(|e| e.name.clone()).collect();
        route_meta.insert("path_names".to_string(), path_names.join(" + "));
        route_meta.insert("candidate_index".to_string(), idx.to_string());

        routes.push(RouteAlloc {
            legs,
            amount_in,
            estimated_out: current,
            min_amount_out: current * slippage_factor,
            metadata: route_meta,
        });
    }

    let total_in: f64 = routes.iter().map(|r| r.amount_in).sum();
    let est_total_out: f64 = routes.iter().map(|r| r.estimated_out).sum();
    let min_total_out: f64 = routes.iter().map(|r| r.min_amount_out).sum();

    let mut plan_meta = HashMap::new();
    plan_meta.insert("algo".to_string(), algo_label.to_string());
    plan_meta.insert("optimization_algorithm".to_string(), algo_label.to_string());
    plan_meta.insert("max_paths".to_string(), constraints.max_paths.to_string());
    plan_meta.insert(
        "slippage_bps".to_string(),
        constraints.slippage_tolerance_bps.to_string(),
    );
    plan_meta.insert("candidate_count".to_string(), candidates.len().to_string());

    if let Some(config) = &constraints.split_config {
        plan_meta.insert(
            "split_precision_bps".to_string(),
            config.min_precision_bps.to_string(),
        );
        plan_meta.insert(
            "split_max_splits".to_string(),
            config.max_splits.to_string(),
        );
    }

    if let Some(extra) = extra_meta {
        plan_meta.extend(extra);
    }

    if routes.is_empty() {
        return RoutePlan::empty(Some(plan_meta));
    }

    RoutePlan {
        routes,
        total_in,
        est_total_out,
        min_total_out,
        metadata: plan_meta,
    }
}

pub fn path_quote(rg: &RoutingGraph, path: &[Edge], amount_in: f64) -> f64 {
    let mut cur_in = amount_in;
    for e in path {
        if let Some(pool) = rg.pools.get(&e.pool_id) {
            cur_in = pool.quote(cur_in).amount_out;
        }
    }
    cur_in
}

pub fn path_marginal_rate<P: AsRef<[Edge]>>(
    rg: &RoutingGraph,
    path: P,
    alloc: f64,
    eps: f64,
) -> f64 {
    let path = path.as_ref();
    let alloc = if alloc < 0.0 { 0.0 } else { alloc };
    let q1 = path_quote(rg, path, alloc);
    let q2 = path_quote(rg, path, alloc + eps);
    ((q2 - q1) / eps).max(1e-18)
}

pub fn apply_virtual_fill(rg: &mut RoutingGraph, path: &[Edge], delta: f64) -> f64 {
    let mut cur_in = delta;
    for e in path {
        if let Some(pool) = rg.pools.get_mut(&e.pool_id) {
            cur_in = pool.apply_virtual_fill(cur_in);
        }
    }
    cur_in
}

#[derive(Debug)]
struct HeapItem {
    neg_marginal_rate: f64,
    path_idx: usize,
    version: usize,
}

impl PartialEq for HeapItem {
    fn eq(&self, other: &Self) -> bool {
        self.neg_marginal_rate == other.neg_marginal_rate
    }
}

impl Eq for HeapItem {}

impl PartialOrd for HeapItem {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        // Note: reversed comparison for max-heap behavior
        other.neg_marginal_rate.partial_cmp(&self.neg_marginal_rate)
    }
}

impl Ord for HeapItem {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap_or(Ordering::Equal)
    }
}

/// Waterfill algorithm for optimal routing
pub fn waterfill(
    rg: &mut RoutingGraph,
    candidates: &[Vec<Edge>],
    amount_in: f64,
    constraints: &RoutingConstraints,
    step_eps: Option<f64>,
) -> RoutePlan {
    if amount_in <= 0.0 || candidates.is_empty() {
        let mut meta = HashMap::new();
        meta.insert("reason".to_string(), "no_candidates".to_string());
        meta.insert("algo".to_string(), "waterfill_v1".to_string());
        return RoutePlan::empty(Some(meta));
    }

    let default_step = determine_step_size(amount_in, constraints, candidates.len());
    let eps = step_eps
        .or(constraints.step_epsilon)
        .unwrap_or(default_step);
    let eps = if eps <= 0.0 {
        default_step
    } else {
        eps.min(amount_in)
    };

    // Build touch map: which paths touch which pools
    let mut touch: HashMap<String, HashSet<usize>> = HashMap::new();
    for (i, p) in candidates.iter().enumerate() {
        for e in p {
            touch
                .entry(e.pool_id.clone())
                .or_insert_with(HashSet::new)
                .insert(i);
        }
    }

    let mut alloc = vec![0.0_f64; candidates.len()];
    let mut route_out = vec![0.0_f64; candidates.len()];
    let mut mu: Vec<f64> = candidates
        .iter()
        .map(|p| path_marginal_rate(rg, p, 0.0_f64, eps))
        .collect();
    let mut ver = vec![0; candidates.len()];
    let mut heap = BinaryHeap::new();

    for i in 0..candidates.len() {
        heap.push(HeapItem {
            neg_marginal_rate: -mu[i],
            path_idx: i,
            version: ver[i],
        });
    }

    let mut remaining = amount_in;
    let mut enabled: HashSet<usize> = HashSet::new();

    while remaining > 1e-12 {
        if heap.is_empty() {
            break;
        }

        let item = heap.pop().unwrap();
        let i = item.path_idx;
        let v = item.version;

        if v != ver[i] {
            continue;
        }

        let m = -item.neg_marginal_rate;
        if m <= 1e-18 {
            break;
        }

        if !enabled.contains(&i) && enabled.len() >= constraints.max_paths {
            ver[i] += 1;
            mu[i] = 0.0;
            heap.push(HeapItem {
                neg_marginal_rate: 0.0,
                path_idx: i,
                version: ver[i],
            });
            continue;
        }

        let delta = eps.min(remaining);

        // Apply virtual fill and get the output
        let gain = apply_virtual_fill(rg, &candidates[i], delta);

        alloc[i] += delta;
        remaining -= delta;
        route_out[i] += gain;
        enabled.insert(i);

        // Update all paths that share pools with this path
        let mut dirty = HashSet::new();
        for e in &candidates[i] {
            if let Some(paths) = touch.get(&e.pool_id) {
                dirty.extend(paths);
            }
        }

        for &j in &dirty {
            let new_mu = path_marginal_rate(rg, &candidates[j], alloc[j], eps);
            mu[j] = new_mu;
            ver[j] += 1;
            heap.push(HeapItem {
                neg_marginal_rate: -new_mu,
                path_idx: j,
                version: ver[j],
            });
        }
    }

    // Build routes
    let mut routes = Vec::new();
    let slippage_factor = (1.0 - constraints.slippage_tolerance_bps / 10_000.0).max(0.0);

    for (i, &amount) in alloc.iter().enumerate() {
        if amount <= 0.0 {
            continue;
        }

        let est = route_out[i];
        let min_out = est * slippage_factor;
        let mut legs = Vec::new();

        for edge in &candidates[i] {
            if let Some(pool) = rg.pools.get(&edge.pool_id) {
                legs.push(RouteLeg {
                    src: edge.src.clone(),
                    dst: edge.dst.clone(),
                    pool_id: edge.pool_id.clone(),
                    pool_kind: pool.kind().to_string(),
                    edge_name: edge.name.clone(),
                    metadata: pool.meta().clone(),
                });
            }
        }

        let mut route_meta = HashMap::new();
        let path_names: Vec<String> = candidates[i].iter().map(|e| e.name.clone()).collect();
        route_meta.insert("path_names".to_string(), path_names.join(" + "));

        routes.push(RouteAlloc {
            legs,
            amount_in: amount,
            estimated_out: est,
            min_amount_out: min_out,
            metadata: route_meta,
        });
    }

    let total_in: f64 = routes.iter().map(|r| r.amount_in).sum();
    let total_est_out: f64 = routes.iter().map(|r| r.estimated_out).sum();
    let total_min_out: f64 = routes.iter().map(|r| r.min_amount_out).sum();

    let mut plan_meta = HashMap::new();
    plan_meta.insert("algo".to_string(), "waterfill_v1".to_string());
    plan_meta.insert(
        "optimization_algorithm".to_string(),
        "waterfill".to_string(),
    );
    plan_meta.insert("step_eps".to_string(), eps.to_string());
    plan_meta.insert("max_paths".to_string(), constraints.max_paths.to_string());
    plan_meta.insert(
        "slippage_bps".to_string(),
        constraints.slippage_tolerance_bps.to_string(),
    );
    plan_meta.insert("candidate_count".to_string(), candidates.len().to_string());
    if let Some(config) = &constraints.split_config {
        plan_meta.insert(
            "split_precision_bps".to_string(),
            config.min_precision_bps.to_string(),
        );
        plan_meta.insert(
            "split_max_splits".to_string(),
            config.max_splits.to_string(),
        );
    }
    plan_meta.extend(constraints.metadata.clone());

    if routes.is_empty() {
        return RoutePlan::empty(Some(plan_meta));
    }

    RoutePlan {
        routes,
        total_in,
        est_total_out: total_est_out,
        min_total_out: total_min_out,
        metadata: plan_meta,
    }
}

pub fn allocate_routes(
    rg: &mut RoutingGraph,
    candidates: &[Vec<Edge>],
    amount_in: f64,
    constraints: &RoutingConstraints,
) -> RoutePlan {
    match constraints.optimization_algorithm {
        OptimizationAlgorithm::Waterfill => waterfill(rg, candidates, amount_in, constraints, None),
        OptimizationAlgorithm::GoldenSection => {
            let allocations = run_golden_section(&*rg, candidates, amount_in, constraints);
            if allocations.iter().all(|&v| v <= MIN_STEP_SIZE) {
                return waterfill(rg, candidates, amount_in, constraints, None);
            }
            let mut meta = HashMap::new();
            let score = evaluate_allocation(&*rg, candidates, &allocations);
            if score.is_finite() {
                meta.insert("allocation_score".to_string(), score.to_string());
            }
            meta.insert(
                "optimization_algorithm".to_string(),
                algo_label(OptimizationAlgorithm::GoldenSection).to_string(),
            );
            apply_allocations_to_graph(
                rg,
                candidates,
                &allocations,
                constraints,
                "golden_section_v1",
                Some(meta),
            )
        }
        OptimizationAlgorithm::Brent => {
            let allocations = run_brent(&*rg, candidates, amount_in, constraints);
            if allocations.iter().all(|&v| v <= MIN_STEP_SIZE) {
                return waterfill(rg, candidates, amount_in, constraints, None);
            }
            let mut meta = HashMap::new();
            let score = evaluate_allocation(&*rg, candidates, &allocations);
            if score.is_finite() {
                meta.insert("allocation_score".to_string(), score.to_string());
            }
            meta.insert(
                "optimization_algorithm".to_string(),
                algo_label(OptimizationAlgorithm::Brent).to_string(),
            );
            apply_allocations_to_graph(
                rg,
                candidates,
                &allocations,
                constraints,
                "brent_v1",
                Some(meta),
            )
        }
        OptimizationAlgorithm::Hybrid => {
            let (allocations, chosen) = run_hybrid(&*rg, candidates, amount_in, constraints);
            if allocations.iter().all(|&v| v <= MIN_STEP_SIZE) {
                return waterfill(rg, candidates, amount_in, constraints, None);
            }
            let mut meta = HashMap::new();
            let score = evaluate_allocation(&*rg, candidates, &allocations);
            if score.is_finite() {
                meta.insert("allocation_score".to_string(), score.to_string());
            }
            meta.insert(
                "hybrid_selected".to_string(),
                algo_label(chosen).to_string(),
            );
            apply_allocations_to_graph(
                rg,
                candidates,
                &allocations,
                constraints,
                "hybrid_v1",
                Some(meta),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::candidates::generate_candidate_paths;
    use crate::sim::demo::build_graph;

    #[test]
    fn test_generate_splits_reaches_total() {
        let config = SplitConfig {
            min_precision_bps: 1,
            max_splits: 512,
            adaptive: true,
        };
        let total = 1_000.0;
        let splits = generate_high_precision_splits(total, &config);
        assert!(!splits.is_empty());
        assert!((splits.last().copied().unwrap() - total).abs() < 1e-6);
    }

    #[test]
    fn test_allocate_routes_golden_section() {
        let mut rg = build_graph(None, None, None);
        let mut constraints = RoutingConstraints::default();
        constraints.max_hops = 3;
        constraints.max_paths = 3;
        constraints.candidate_pool_size = 8;
        constraints.optimization_algorithm = OptimizationAlgorithm::GoldenSection;
        constraints.split_config = Some(SplitConfig {
            min_precision_bps: 1,
            max_splits: 512,
            adaptive: true,
        });

        let candidates = generate_candidate_paths(&rg, "A", "B", &constraints);
        assert!(!candidates.is_empty());

        let plan = allocate_routes(&mut rg, &candidates, 400.0, &constraints);
        assert!(!plan.routes.is_empty());
        assert!(plan.est_total_out > 0.0);
        assert_eq!(
            plan.metadata.get("algo").map(|s| s.as_str()),
            Some("golden_section_v1")
        );
    }

    #[test]
    fn test_allocate_routes_brent() {
        let mut rg = build_graph(None, None, None);
        let mut constraints = RoutingConstraints::default();
        constraints.max_hops = 3;
        constraints.max_paths = 3;
        constraints.candidate_pool_size = 8;
        constraints.optimization_algorithm = OptimizationAlgorithm::Brent;
        constraints.split_config = Some(SplitConfig {
            min_precision_bps: 2,
            max_splits: 256,
            adaptive: true,
        });

        let candidates = generate_candidate_paths(&rg, "A", "B", &constraints);
        assert!(!candidates.is_empty());

        let plan = allocate_routes(&mut rg, &candidates, 450.0, &constraints);
        assert!(!plan.routes.is_empty());
        assert!(plan.est_total_out > 0.0);
        assert_eq!(
            plan.metadata.get("algo").map(|s| s.as_str()),
            Some("brent_v1")
        );
    }
}
