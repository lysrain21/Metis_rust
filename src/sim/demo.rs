use crate::core::candidates::generate_candidate_paths;
use crate::core::constraints::RoutingConstraints;
use crate::core::graph::RoutingGraph;
use crate::core::splitter::{path_marginal_rate, waterfill};
use crate::{CLMMSimpleApprox, CLOBTopN, CPMMPool};

pub fn build_graph(
    ab_big_y: Option<f64>,
    ab_big_fee: Option<f64>,
    clob_steps: Option<Vec<(f64, f64)>>,
) -> RoutingGraph {
    let mut rg = RoutingGraph::new();

    // A->B three sources: big CPMM, small CPMM, one two-hop via X (A->X CLMM, X->B CLOB)
    let big_y = ab_big_y.unwrap_or(10_000.0);
    let big_fee = ab_big_fee.unwrap_or(0.003);

    rg.add_pool_edge(
        "A",
        "B",
        Box::new(CPMMPool::new(
            "cpmm_big".to_string(),
            10_000.0,
            big_y,
            big_fee,
        )),
        "AB_big",
    );

    rg.add_pool_edge(
        "A",
        "B",
        Box::new(CPMMPool::new(
            "cpmm_small".to_string(),
            1_000.0,
            1_200.0,
            0.003,
        )),
        "AB_small",
    );

    rg.add_pool_edge(
        "A",
        "X",
        Box::new(CLMMSimpleApprox::new(
            "clmm_ax".to_string(),
            5_000.0,
            4_900.0,
            0.003,
        )),
        "AX_clmm",
    );

    let steps = clob_steps.unwrap_or_else(|| vec![(1.010, 300.0), (1.005, 500.0), (1.000, 1e9)]);

    rg.add_pool_edge(
        "X",
        "B",
        Box::new(CLOBTopN::new("clob_xb".to_string(), steps, 0.0)),
        "XB_clob",
    );

    rg
}

pub fn run_scenario(title: &str, rg: &mut RoutingGraph, amount_in: f64, verbose: bool) {
    let src = "A";
    let dst = "B";
    let mut constraints = RoutingConstraints::default();
    constraints.max_hops = 3;
    constraints.max_paths = 4;
    constraints.candidate_pool_size = 12;

    let cand_paths = generate_candidate_paths(rg, src, dst, &constraints);
    let step_eps = constraints.step_epsilon.unwrap_or(amount_in / 200.0);

    if verbose {
        println!("-- edges present --");
        for ((s, d, _), e) in &rg.edges {
            if s == "A" && d == "B" {
                if let Some(pool) = rg.pools.get(&e.pool_id) {
                    let mu = pool.marginal_rate(0.0);
                    println!("  edge {}: mu0={:.6} kind={}", e.name, mu, pool.kind());
                }
            }
        }
        println!("-- candidates & initial μ --");
        for (idx, p) in cand_paths.iter().enumerate() {
            let names: Vec<String> = p.iter().map(|e| e.name.clone()).collect();
            let mu0 = path_marginal_rate(rg, p, 0.0, step_eps);
            println!("  [{}] mu0={:.6} | {}", idx, mu0, names.join(" + "));
        }
    }

    let plan = waterfill(rg, &cand_paths, amount_in, &constraints, Some(step_eps));

    println!("== {} ==", title);
    println!(
        "total_in={:.6}  est_total_out={:.6}  min_total_out={:.6}",
        plan.total_in, plan.est_total_out, plan.min_total_out
    );

    for (i, r) in plan.routes.iter().enumerate() {
        let legs = &r.legs;
        let names: Vec<String> = legs.iter().map(|leg| leg.edge_name.clone()).collect();
        let mut nodes = vec![legs[0].src.clone()];
        nodes.extend(legs.iter().map(|leg| leg.dst.clone()));

        println!(
            "[Route {}] in={:.6}  est_out={:.6}  min_out={:.6}",
            i + 1,
            r.amount_in,
            r.estimated_out,
            r.min_amount_out
        );
        println!(
            "          path: {} | {}",
            nodes.join(" -> "),
            names.join(" + ")
        );
    }
}
