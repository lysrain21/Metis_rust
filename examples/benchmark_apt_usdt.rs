use metis::{
    benchmark::comparison::{create_mock_onchain_metrics, BenchmarkRunner},
    core::{constraints::RoutingConstraints, graph::RoutingGraph},
    loader::pool_loader::PoolFactory,
};

fn main() {
    println!("\n╔════════════════════════════════════════════════════════════════╗");
    println!("║           Metis Router Performance Benchmark                   ║");
    println!("║                   APT → USDT Swap Test                         ║");
    println!("╚════════════════════════════════════════════════════════════════╝\n");

    // Load pools from JSON configuration
    println!("📂 Loading pool configurations from JSON...");
    let pools = PoolFactory::load_from_file("examples/pools/apt_usdt_pools.json")
        .expect("Failed to load pools from JSON");

    println!("✅ Loaded {} pools successfully!\n", pools.len());

    // Build routing graph
    println!("🔨 Building routing graph...");
    let mut graph = RoutingGraph::new();

    for (i, pool) in pools.into_iter().enumerate() {
        let pool_id = pool.id().to_string();
        let pool_kind = pool.kind().to_string();

        // Add pool to graph
        graph.add_pool_edge("APT", "USDT", pool, &format!("edge_{}", i));
        println!("  ├─ Added pool: {} ({})", pool_id, pool_kind);
    }
    println!("✅ Graph built with {} pools\n", graph.pools.len());

    // Set up routing constraints
    let constraints = RoutingConstraints {
        max_hops: 3,
        max_paths: 4,
        candidate_pool_size: 10,
        intermediate_token_whitelist: None,
        slippage_tolerance_bps: 50.0,
        step_epsilon: None,
        metadata: Default::default(),
        ..RoutingConstraints::default()
    };

    // Create benchmark runner
    let mut runner = BenchmarkRunner::new(graph, Some(constraints.clone()));

    // Test parameters
    let input_amount = 10000.0;
    println!("🔄 Swap Parameters:");
    println!("  ├─ From: APT");
    println!("  ├─ To: USDT");
    println!("  └─ Amount: {}\n", input_amount);

    // Simulate "on-chain" data (in a real scenario, you'd fetch this from the blockchain)
    println!("⛓️  Simulating on-chain swap data...");
    let on_chain_output = 49500.0; // Assume on-chain got this output
    let on_chain_execution_time_us = 25000; // Assume 25ms on-chain

    let on_chain_metrics =
        create_mock_onchain_metrics(input_amount, on_chain_output, on_chain_execution_time_us);

    println!("✅ On-chain data simulated");
    println!("  ├─ Output: {}", on_chain_output);
    println!("  └─ Time: {} μs\n", on_chain_execution_time_us);

    // Run local simulation
    println!("🖥️  Running local Metis simulation...");
    runner.print_comparison_report("APT", "USDT", input_amount, on_chain_metrics);

    println!("\n✨ Benchmark complete!\n");

    // Additional detailed simulation run
    println!("═══════════════════════════════════════════════════════════════\n");
    println!("📊 Detailed Route Analysis:\n");

    // Reset graph for another run
    let pools2 = PoolFactory::load_from_file("examples/pools/apt_usdt_pools.json")
        .expect("Failed to load pools");

    let mut graph2 = RoutingGraph::new();
    for (i, pool) in pools2.into_iter().enumerate() {
        graph2.add_pool_edge("APT", "USDT", pool, &format!("edge_{}", i));
    }

    let mut runner2 = BenchmarkRunner::new(graph2, Some(constraints.clone()));
    let (plan, metrics) = runner2.run_simulation("APT", "USDT", input_amount);

    println!("Route Plan Details:");
    println!("  ├─ Total Routes: {}", plan.routes.len());
    println!("  ├─ Total Input: {}", plan.total_in);
    println!("  ├─ Total Output: {}", plan.est_total_out);
    println!("  └─ Estimated Min Output: {}\n", plan.min_total_out);

    for (i, route_alloc) in plan.routes.iter().enumerate() {
        println!("  Route #{}:", i + 1);
        println!("    ├─ Allocation: {:.2}", route_alloc.amount_in);
        println!("    ├─ Estimated Output: {:.2}", route_alloc.estimated_out);
        println!("    └─ Path:");
        for (j, leg) in route_alloc.legs.iter().enumerate() {
            println!(
                "        {}─ {} → {} via pool '{}'",
                if j == route_alloc.legs.len() - 1 {
                    "└"
                } else {
                    "├"
                },
                leg.src,
                leg.dst,
                leg.pool_id
            );
        }
        println!();
    }

    println!("{}", metrics.format());
    println!("\n═══════════════════════════════════════════════════════════════\n");
}
