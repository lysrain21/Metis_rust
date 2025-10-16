use crate::core::pool::{Edge, PoolLike};
use petgraph::graph::{DiGraph, NodeIndex};
use std::collections::HashMap;

/// RoutingGraph represents the token swap graph
pub struct RoutingGraph {
    pub g: DiGraph<String, (String, String, String)>, // node: token, edge: (src, dst, name)
    pub edges: HashMap<(String, String, String), Edge>,
    pub pools: HashMap<String, Box<dyn PoolLike>>,
    node_map: HashMap<String, NodeIndex>,
}

impl RoutingGraph {
    pub fn new() -> Self {
        Self {
            g: DiGraph::new(),
            edges: HashMap::new(),
            pools: HashMap::new(),
            node_map: HashMap::new(),
        }
    }

    fn get_or_create_node(&mut self, token: &str) -> NodeIndex {
        if let Some(&idx) = self.node_map.get(token) {
            idx
        } else {
            let idx = self.g.add_node(token.to_string());
            self.node_map.insert(token.to_string(), idx);
            idx
        }
    }

    pub fn add_pool_edge(&mut self, src: &str, dst: &str, pool: Box<dyn PoolLike>, name: &str) {
        let pool_id = pool.id().to_string();
        self.pools.insert(pool_id.clone(), pool);

        let key = (src.to_string(), dst.to_string(), name.to_string());
        let edge = Edge::new(
            src.to_string(),
            dst.to_string(),
            pool_id,
            name.to_string(),
        );
        self.edges.insert(key.clone(), edge);

        let src_idx = self.get_or_create_node(src);
        let dst_idx = self.get_or_create_node(dst);
        self.g.add_edge(src_idx, dst_idx, key);
    }

    pub fn outgoing(&self, node: &str) -> Vec<Edge> {
        let mut result = Vec::new();
        if let Some(&node_idx) = self.node_map.get(node) {
            let mut edges = self.g.neighbors_directed(node_idx, petgraph::Direction::Outgoing);
            while let Some(_) = edges.next() {
                // Get the edge data
                let mut walker = self
                    .g
                    .neighbors_directed(node_idx, petgraph::Direction::Outgoing)
                    .detach();
                while let Some((edge_idx, _)) = walker.next(&self.g) {
                    if let Some(key) = self.g.edge_weight(edge_idx) {
                        if let Some(edge) = self.edges.get(key) {
                            result.push(edge.clone());
                        }
                    }
                }
                break;
            }
        }
        result
    }

    pub fn get_node_index(&self, token: &str) -> Option<NodeIndex> {
        self.node_map.get(token).copied()
    }

    pub fn nodes(&self) -> Vec<String> {
        self.g.node_weights().cloned().collect()
    }
}

impl Default for RoutingGraph {
    fn default() -> Self {
        Self::new()
    }
}
