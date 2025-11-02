use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet};

use crate::core::constraints::RoutingConstraints;
use crate::core::graph::RoutingGraph;
use crate::core::pool::Edge;

const WEIGHT_TINY: f64 = 1e-6;
const MERGE_TOLERANCE: f64 = 1e-9;

#[derive(Clone)]
struct RouteState {
    current: String,
    path: Vec<Edge>,
    visited: HashSet<String>,
    weight: f64,
    hops: usize,
}

impl RouteState {
    fn new(src: &str) -> Self {
        let mut visited = HashSet::new();
        visited.insert(src.to_string());
        Self {
            current: src.to_string(),
            path: Vec::new(),
            visited,
            weight: 0.0,
            hops: 0,
        }
    }

    fn extend(&self, edge: &Edge, weight: f64) -> Self {
        let mut path = self.path.clone();
        path.push(edge.clone());

        let mut visited = self.visited.clone();
        visited.insert(edge.dst.clone());

        Self {
            current: edge.dst.clone(),
            path,
            visited,
            weight: self.weight + weight,
            hops: self.hops + 1,
        }
    }
}

#[derive(Clone)]
struct HeapItem {
    neg_weight: f64,
    state: RouteState,
}

impl PartialEq for HeapItem {
    fn eq(&self, other: &Self) -> bool {
        self.neg_weight == other.neg_weight
    }
}

impl Eq for HeapItem {}

impl PartialOrd for HeapItem {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        other.neg_weight.partial_cmp(&self.neg_weight)
    }
}

impl Ord for HeapItem {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap_or(Ordering::Equal)
    }
}

/// Calculate an additive weight for an edge based on its marginal rate.
pub(crate) fn rate_weight(rg: &RoutingGraph, pool_id: &str, tiny: f64) -> f64 {
    if let Some(pool) = rg.pools.get(pool_id) {
        let rate = pool
            .marginal_rate(0.0)
            .max(pool.quote(tiny).effective_rate)
            .max(1e-18);
        -rate.ln()
    } else {
        f64::INFINITY
    }
}

/// Helper structure responsible for streaming path generation.
pub struct StreamingRouteBuilder<'a> {
    graph: &'a RoutingGraph,
    src: &'a str,
    dst: &'a str,
    constraints: &'a RoutingConstraints,
    queue: BinaryHeap<HeapItem>,
    per_node_cache: HashMap<String, Vec<f64>>,
    max_states_per_node: usize,
}

impl<'a> StreamingRouteBuilder<'a> {
    pub fn new(
        graph: &'a RoutingGraph,
        src: &'a str,
        dst: &'a str,
        constraints: &'a RoutingConstraints,
    ) -> Self {
        let mut queue = BinaryHeap::new();
        queue.push(HeapItem {
            neg_weight: 0.0,
            state: RouteState::new(src),
        });

        let max_states_per_node = constraints.max_paths.max(2) * 3;

        Self {
            graph,
            src,
            dst,
            constraints,
            queue,
            per_node_cache: HashMap::new(),
            max_states_per_node,
        }
    }

    /// Determine whether a newly generated state should be kept by applying
    /// a lightweight merge policy that retains only the best few states per node.
    fn merge_routes_at_state(&mut self, state: &RouteState) -> bool {
        let entry = self
            .per_node_cache
            .entry(state.current.clone())
            .or_insert_with(Vec::new);

        entry.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));

        if entry.len() < self.max_states_per_node {
            entry.push(state.weight);
            true
        } else {
            // Keep the state if it improves on the worst cached weight.
            if let Some(worst) = entry.last().copied() {
                if state.weight + MERGE_TOLERANCE < worst {
                    entry.pop();
                    entry.push(state.weight);
                    true
                } else {
                    false
                }
            } else {
                entry.push(state.weight);
                true
            }
        }
    }

    fn expand_state(&mut self, state: RouteState) {
        if state.hops >= self.constraints.max_hops {
            return;
        }

        for edge in self.graph.outgoing(&state.current) {
            if !self
                .constraints
                .is_allowed_node(&edge.dst, self.src, self.dst)
            {
                continue;
            }
            if state.visited.contains(&edge.dst) {
                continue;
            }

            let weight = rate_weight(self.graph, &edge.pool_id, WEIGHT_TINY);
            if !weight.is_finite() {
                continue;
            }

            let next_state = state.extend(&edge, weight);
            if !self.merge_routes_at_state(&next_state) {
                continue;
            }

            self.queue.push(HeapItem {
                neg_weight: -next_state.weight,
                state: next_state,
            });
        }
    }

    /// Collect the top-N candidate paths in a streaming manner.
    pub fn collect(mut self, limit: usize) -> Vec<Vec<Edge>> {
        let mut results = Vec::new();

        while let Some(item) = self.queue.pop() {
            let state = item.state;
            if !state.path.is_empty() && state.current == self.dst {
                results.push(state.path.clone());
                if results.len() >= limit {
                    break;
                }
            }

            self.expand_state(state);
        }

        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::demo::build_graph;

    #[test]
    fn test_streaming_builder_finds_paths() {
        let rg = build_graph(None, None, None);
        let mut constraints = RoutingConstraints::default();
        constraints.max_hops = 3;
        constraints.max_paths = 3;
        constraints.candidate_pool_size = 6;

        let builder = StreamingRouteBuilder::new(&rg, "A", "B", &constraints);
        let paths = builder.collect(4);
        assert!(!paths.is_empty());
        assert!(paths.len() <= 4);
        for path in &paths {
            assert!(!path.is_empty());
            assert_eq!(path.first().unwrap().src, "A");
            assert_eq!(path.last().unwrap().dst, "B");
        }
    }
}
