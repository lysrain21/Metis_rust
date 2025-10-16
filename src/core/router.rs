use crate::core::candidates::generate_candidate_paths;
use crate::core::constraints::RoutingConstraints;
use crate::core::graph::RoutingGraph;
use crate::core::plan::RoutePlan;
use crate::core::splitter::waterfill;

/// Main entry point for routing
pub fn plan_routes(
    rg: &mut RoutingGraph,
    src_token: &str,
    dst_token: &str,
    amount_in: f64,
    constraints: Option<RoutingConstraints>,
) -> RoutePlan {
    let constraints = constraints.unwrap_or_default();
    let candidates = generate_candidate_paths(rg, src_token, dst_token, &constraints);
    waterfill(rg, &candidates, amount_in, &constraints, None)
}
