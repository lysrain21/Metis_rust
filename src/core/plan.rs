use std::collections::HashMap;

/// RouteLeg represents one hop in a route
#[derive(Debug, Clone)]
pub struct RouteLeg {
    pub src: String,
    pub dst: String,
    pub pool_id: String,
    pub pool_kind: String,
    pub edge_name: String,
    pub metadata: HashMap<String, String>,
}

/// RouteAlloc represents a single route with its allocation
#[derive(Debug, Clone)]
pub struct RouteAlloc {
    pub legs: Vec<RouteLeg>,
    pub amount_in: f64,
    pub estimated_out: f64,
    pub min_amount_out: f64,
    pub metadata: HashMap<String, String>,
}

/// RoutePlan is the final routing plan with multiple routes
#[derive(Debug, Clone)]
pub struct RoutePlan {
    pub routes: Vec<RouteAlloc>,
    pub total_in: f64,
    pub est_total_out: f64,
    pub min_total_out: f64,
    pub metadata: HashMap<String, String>,
}

impl RoutePlan {
    pub fn empty(metadata: Option<HashMap<String, String>>) -> Self {
        Self {
            routes: Vec::new(),
            total_in: 0.0,
            est_total_out: 0.0,
            min_total_out: 0.0,
            metadata: metadata.unwrap_or_default(),
        }
    }

    pub fn legs(&self) -> Vec<Vec<RouteLeg>> {
        self.routes.iter().map(|r| r.legs.clone()).collect()
    }

    pub fn allocations(&self) -> Vec<f64> {
        self.routes.iter().map(|r| r.amount_in).collect()
    }

    pub fn min_out(&self) -> Vec<f64> {
        self.routes.iter().map(|r| r.min_amount_out).collect()
    }
}
