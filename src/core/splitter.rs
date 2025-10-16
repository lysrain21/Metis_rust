use crate::core::constraints::RoutingConstraints;
use crate::core::graph::RoutingGraph;
use crate::core::plan::{RouteAlloc, RouteLeg, RoutePlan};
use crate::core::pool::Edge;
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet};

pub fn path_quote(rg: &RoutingGraph, path: &[Edge], amount_in: f64) -> f64 {
    let mut cur_in = amount_in;
    for e in path {
        if let Some(pool) = rg.pools.get(&e.pool_id) {
            cur_in = pool.quote(cur_in).amount_out;
        }
    }
    cur_in
}

pub fn path_marginal_rate<P: AsRef<[Edge]>>(rg: &RoutingGraph, path: P, alloc: f64, eps: f64) -> f64 {
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
        other
            .neg_marginal_rate
            .partial_cmp(&self.neg_marginal_rate)
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

    let eps = step_eps
        .or(constraints.step_epsilon)
        .unwrap_or(amount_in / 200.0);
    let eps = if eps <= 0.0 { amount_in } else { eps };

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
    plan_meta.insert("step_eps".to_string(), eps.to_string());
    plan_meta.insert("max_paths".to_string(), constraints.max_paths.to_string());
    plan_meta.insert(
        "slippage_bps".to_string(),
        constraints.slippage_tolerance_bps.to_string(),
    );
    plan_meta.insert(
        "candidate_count".to_string(),
        candidates.len().to_string(),
    );
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
