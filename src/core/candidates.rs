use crate::core::constraints::RoutingConstraints;
use crate::core::graph::RoutingGraph;
use crate::core::incremental::{rate_weight, StreamingRouteBuilder};
use crate::core::pool::Edge;
use std::collections::{HashMap, HashSet};

const BF_TINY: f64 = 1e-6;

/// Bellman-Ford baseline algorithm to find the theoretically optimal path
pub fn bf_baseline(
    rg: &RoutingGraph,
    src: &str,
    dst: &str,
    constraints: &RoutingConstraints,
) -> (Vec<Edge>, f64) {
    let nodes = rg.nodes();
    let mut dist: HashMap<String, f64> = HashMap::new();
    let mut hops: HashMap<String, usize> = HashMap::new();
    let mut prev: HashMap<String, (String, Edge)> = HashMap::new();

    for node in &nodes {
        dist.insert(node.clone(), f64::INFINITY);
        hops.insert(node.clone(), usize::MAX);
    }
    dist.insert(src.to_string(), 0.0);
    hops.insert(src.to_string(), 0);

    for _ in 0..constraints.max_hops {
        let mut updated = false;
        for u in &nodes {
            let cur_dist = *dist.get(u).unwrap_or(&f64::INFINITY);
            if cur_dist == f64::INFINITY {
                continue;
            }
            let cur_hops = *hops.get(u).unwrap_or(&usize::MAX);
            if cur_hops >= constraints.max_hops {
                continue;
            }

            for e in rg.outgoing(u) {
                if !constraints.is_allowed_node(&e.dst, src, dst) {
                    continue;
                }
                let w = rate_weight(rg, &e.pool_id, BF_TINY);
                let cand_dist = cur_dist + w;
                let cand_hops = cur_hops + 1;
                if cand_hops > constraints.max_hops {
                    continue;
                }

                let current_dist = *dist.get(&e.dst).unwrap_or(&f64::INFINITY);
                let current_hops = *hops.get(&e.dst).unwrap_or(&usize::MAX);

                // Prioritize lower distance (higher rate). Use hops as tie-breaker.
                if cand_dist < current_dist - 1e-9
                    || ((cand_dist - current_dist).abs() < 1e-9 && cand_hops < current_hops)
                {
                    dist.insert(e.dst.clone(), cand_dist);
                    prev.insert(e.dst.clone(), (u.clone(), e.clone()));
                    hops.insert(e.dst.clone(), cand_hops);
                    updated = true;
                }
            }
        }
        if !updated {
            break;
        }
    }

    if !prev.contains_key(dst) {
        return (Vec::new(), f64::INFINITY);
    }

    let mut path = Vec::new();
    let mut cur = dst.to_string();
    while cur != src {
        if let Some((u, e)) = prev.get(&cur) {
            path.push(e.clone());
            cur = u.clone();
        } else {
            break;
        }
    }
    path.reverse();
    let final_dist = *dist.get(dst).unwrap_or(&f64::INFINITY);
    (path, final_dist)
}

/// Simple K-shortest paths implementation (simplified version of Yen's algorithm)
/// For production, consider using petgraph's builtin algorithms or a more robust implementation
pub fn yen_k_paths(
    rg: &RoutingGraph,
    src: &str,
    dst: &str,
    constraints: &RoutingConstraints,
) -> Vec<Vec<Edge>> {
    // This is a simplified version. A full Yen's K-shortest paths would be more complex.
    // For now, we'll use a DFS-based approach to find multiple paths
    let mut paths = Vec::new();
    let mut visited_paths: HashSet<Vec<String>> = HashSet::new();

    fn dfs(
        rg: &RoutingGraph,
        current: &str,
        dst: &str,
        path: &mut Vec<Edge>,
        visited: &mut HashSet<String>,
        paths: &mut Vec<Vec<Edge>>,
        visited_paths: &mut HashSet<Vec<String>>,
        constraints: &RoutingConstraints,
        src: &str,
    ) {
        if current == dst {
            let node_path: Vec<String> = {
                let mut np = vec![src.to_string()];
                for e in path.iter() {
                    np.push(e.dst.clone());
                }
                np
            };
            if !visited_paths.contains(&node_path) {
                visited_paths.insert(node_path);
                paths.push(path.clone());
            }
            return;
        }

        if path.len() >= constraints.max_hops {
            return;
        }

        if paths.len() >= constraints.candidate_pool_size {
            return;
        }

        for edge in rg.outgoing(current) {
            if visited.contains(&edge.dst) {
                continue;
            }
            if !constraints.is_allowed_node(&edge.dst, src, dst) {
                continue;
            }

            visited.insert(edge.dst.clone());
            path.push(edge.clone());
            dfs(
                rg,
                &edge.dst,
                dst,
                path,
                visited,
                paths,
                visited_paths,
                constraints,
                src,
            );
            path.pop();
            visited.remove(&edge.dst);
        }
    }

    let mut visited = HashSet::new();
    visited.insert(src.to_string());
    let mut path = Vec::new();
    dfs(
        rg,
        src,
        dst,
        &mut path,
        &mut visited,
        &mut paths,
        &mut visited_paths,
        constraints,
        src,
    );

    paths
}

fn dedupe_paths(paths: Vec<Vec<Edge>>) -> Vec<Vec<Edge>> {
    let mut seen: HashSet<Vec<(String, String, String)>> = HashSet::new();
    let mut result = Vec::new();

    for p in paths {
        let key: Vec<(String, String, String)> = p
            .iter()
            .map(|e| (e.src.clone(), e.dst.clone(), e.name.clone()))
            .collect();
        if !seen.contains(&key) {
            seen.insert(key);
            result.push(p);
        }
    }
    result
}

/// Generate candidate paths for routing
pub fn generate_candidate_paths(
    rg: &RoutingGraph,
    src: &str,
    dst: &str,
    constraints: &RoutingConstraints,
) -> Vec<Vec<Edge>> {
    let max_candidates = constraints.max_paths.max(constraints.candidate_pool_size);
    if max_candidates == 0 {
        return Vec::new();
    }

    let mut merged: Vec<Vec<Edge>> = Vec::new();

    // Primary: streaming builder (incremental route construction)
    let streaming = StreamingRouteBuilder::new(rg, src, dst, constraints).collect(max_candidates);
    merged.extend(streaming);

    // Ensure the Bellman-Ford baseline path is always part of the candidate set
    let (base, _) = bf_baseline(rg, src, dst, constraints);
    if !base.is_empty() {
        merged.push(base);
    }

    // Include direct edges if they exist (cheap and often optimal)
    for ((s, d, _), e) in &rg.edges {
        if s == src && d == dst {
            merged.push(vec![e.clone()]);
        }
    }

    // Fall back to DFS-based K paths if we still need more diversity
    if merged.len() < max_candidates {
        let yen_paths = yen_k_paths(rg, src, dst, constraints);
        merged.extend(yen_paths);
    }

    let merged = dedupe_paths(merged);
    merged.into_iter().take(max_candidates).collect()
}
