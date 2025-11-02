use metis::core::constraints::{OptimizationAlgorithm, RoutingConstraints, SplitConfig};
use metis::core::router::plan_routes;
use metis::sim::demo::build_graph;
use std::collections::HashSet;

#[test]
fn test_plan_routes_structure() {
    let mut rg = build_graph(None, None, None);
    let mut constraints = RoutingConstraints::default();
    constraints.max_hops = 3;
    constraints.max_paths = 3;
    constraints.candidate_pool_size = 8;
    constraints.slippage_tolerance_bps = 0.0;

    let plan = plan_routes(&mut rg, "A", "B", 500.0, Some(constraints.clone()));

    assert!(
        !plan.routes.is_empty(),
        "expected at least one route allocation"
    );
    assert_eq!(plan.routes.len(), plan.legs().len());
    assert_eq!(plan.routes.len(), plan.allocations().len());

    let sum_allocs: f64 = plan.allocations().iter().sum();
    assert!(
        (sum_allocs - plan.total_in).abs() < 1e-9,
        "allocations sum should match total_in"
    );

    let sum_est_out: f64 = plan.routes.iter().map(|r| r.estimated_out).sum();
    assert!(
        (sum_est_out - plan.est_total_out).abs() < 1e-9,
        "estimated_out sum should match est_total_out"
    );

    assert_eq!(
        plan.metadata.get("algo").map(|s| s.as_str()),
        Some("waterfill_v1")
    );

    for r in &plan.routes {
        assert!(
            (r.estimated_out - r.min_amount_out).abs() < 1e-9,
            "with 0 slippage, estimated_out should equal min_amount_out"
        );
    }
}

#[test]
fn test_intermediate_whitelist_blocks_non_allowed_nodes() {
    let mut rg = build_graph(None, None, None);
    let mut constraints = RoutingConstraints::default();
    constraints.max_hops = 3;
    constraints.max_paths = 3;
    constraints.candidate_pool_size = 8;
    constraints.intermediate_token_whitelist = Some(HashSet::new()); // Block all intermediates

    let plan = plan_routes(&mut rg, "A", "B", 400.0, Some(constraints));

    assert!(
        !plan.routes.is_empty(),
        "direct edge should remain even with tight whitelist"
    );

    for route in &plan.routes {
        // All intermediate nodes forbidden, only direct A->B edges allowed
        for leg in &route.legs {
            assert_ne!(
                leg.dst, "X",
                "intermediate node X should be blocked by whitelist"
            );
        }
    }
}

#[test]
fn test_max_paths_limit_enforced() {
    let mut rg = build_graph(None, None, None);
    let mut constraints = RoutingConstraints::default();
    constraints.max_hops = 3;
    constraints.max_paths = 1;
    constraints.candidate_pool_size = 12;

    let plan = plan_routes(&mut rg, "A", "B", 600.0, Some(constraints.clone()));

    assert!(
        plan.routes.len() <= constraints.max_paths,
        "routes count should not exceed max_paths"
    );
    assert_eq!(
        plan.metadata
            .get("max_paths")
            .and_then(|s| s.parse::<usize>().ok()),
        Some(constraints.max_paths)
    );
}

#[test]
fn test_plan_routes_with_golden_section_algorithm() {
    let mut rg = build_graph(None, None, None);
    let mut constraints = RoutingConstraints::default();
    constraints.max_hops = 3;
    constraints.max_paths = 3;
    constraints.candidate_pool_size = 6;
    constraints.optimization_algorithm = OptimizationAlgorithm::GoldenSection;
    constraints.split_config = Some(SplitConfig {
        min_precision_bps: 1,
        max_splits: 512,
        adaptive: true,
    });

    let plan = plan_routes(&mut rg, "A", "B", 550.0, Some(constraints));
    assert!(!plan.routes.is_empty());
    assert_eq!(
        plan.metadata.get("algo").map(|s| s.as_str()),
        Some("golden_section_v1")
    );
    assert_eq!(
        plan.metadata
            .get("optimization_algorithm")
            .map(|s| s.as_str()),
        Some("golden_section")
    );
}
