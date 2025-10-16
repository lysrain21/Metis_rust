use std::collections::HashMap;

/// Quote result from a pool
#[derive(Debug, Clone)]
pub struct Quote {
    pub amount_out: f64,
    pub effective_rate: f64, // amount_out / amount_in
    pub fee: f64,
}

/// Edge represents a directed connection between two tokens through a pool
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Edge {
    pub src: String,
    pub dst: String,
    pub pool_id: String,
    pub name: String,
}

impl Edge {
    pub fn new(src: String, dst: String, pool_id: String, name: String) -> Self {
        Self {
            src,
            dst,
            pool_id,
            name,
        }
    }
}

/// PoolLike trait defines the interface for all pool types
pub trait PoolLike {
    fn id(&self) -> &str;
    fn kind(&self) -> &str;
    fn meta(&self) -> &HashMap<String, String>;

    /// Get a quote for swapping amount_in
    fn quote(&self, amount_in: f64) -> Quote;

    /// Get the marginal rate at a given amount_in
    fn marginal_rate(&self, amount_in: f64) -> f64;

    /// Apply a virtual fill and return the output amount
    /// This modifies the pool's internal state
    fn apply_virtual_fill(&mut self, amount_in: f64) -> f64;

    /// Clone the pool into a boxed trait object
    fn clone_box(&self) -> Box<dyn PoolLike>;
}

// Implement Clone for Box<dyn PoolLike>
impl Clone for Box<dyn PoolLike> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}
