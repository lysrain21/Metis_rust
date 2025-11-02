use metis::sim::demo::{build_graph, run_scenario};

fn main() {
    // Scenario 1: Tune AB_big's initial price/fee, observe "early allocation, later yield"
    let mut rg1 = build_graph(Some(15_000.0), Some(0.0025), None);
    run_scenario(
        "Scenario 1: AB_big tuned (y=15000, fee=0.0025)",
        &mut rg1,
        500.0,
        true,
    );
    println!();

    // Scenario 2a: Increase CLOB first level size, observe rebalancing between two routes
    let mut rg2a = build_graph(
        None,
        None,
        Some(vec![(1.010, 800.0), (1.005, 500.0), (1.000, 1e9)]),
    );
    run_scenario(
        "Scenario 2a: CLOB L1 size=800 @1.010",
        &mut rg2a,
        500.0,
        false,
    );
    println!();

    // Scenario 2b: Lower CLOB first level price to 1.005, observe rebalancing
    let mut rg2b = build_graph(
        None,
        None,
        Some(vec![(1.005, 300.0), (1.003, 500.0), (1.000, 1e9)]),
    );
    run_scenario(
        "Scenario 2b: CLOB L1 price=1.005 (tighter)",
        &mut rg2b,
        500.0,
        false,
    );
}
