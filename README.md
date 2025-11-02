# Metis - DeFi Routing Optimizer

A Rust implementation of a sophisticated DeFi routing optimizer that finds optimal trading paths across multiple liquidity sources using streaming path discovery and configurable allocation algorithms (waterfill, golden-section, Brent).

## Overview

Metis is a routing engine that optimizes token swaps across heterogeneous liquidity pools (CPMM, CLMM, CLOB, Curve-style stable pools) by:
- Incrementally discovering best paths using a streaming variant of Bellman-Ford with dynamic pruning
- Allocating capital across paths using configurable optimizers (waterfill, golden-section search, Brent search, or hybrid)
- Supporting complex multi-hop routes with configurable constraints

## Features

- **Multiple Pool Types**:
  - CPMM (Constant Product Market Maker) - e.g., Uniswap V2
  - CLMM (Concentrated Liquidity Market Maker) - e.g., Uniswap V3
  - CLOB (Central Limit Order Book) - order book with multiple price levels
  - Curve-style stable swap pools with amplification-based invariant

- **Advanced Routing**:
  - Streaming Bellman-Ford search with incremental candidate generation and deduplication
  - DFS-based K-shortest path fallback for additional diversity
  - Waterfill, golden-section, Brent, and hybrid allocation strategies
  - High-precision split control with adaptive step sizing
  - Virtual fill simulation to account for pool state changes

- **Flexible Configuration**:
  - Maximum hops per path
  - Maximum number of concurrent paths
  - Intermediate token whitelisting
  - Slippage tolerance control
  - Customizable split precision (bps) and allocation algorithm selection

## Project Structure

```
Metis_rust/
├── src/
│   ├── lib.rs                 # Library entry point
│   ├── core/
│   │   ├── pool.rs           # PoolLike trait, Quote, Edge
│   │   ├── graph.rs          # RoutingGraph using petgraph
│   │   ├── constraints.rs    # Routing configuration
│   │   ├── candidates.rs     # Path finding algorithms
│   │   ├── incremental.rs    # Streaming route builder
│   │   ├── optimization.rs   # Continuous optimizers for splits
│   │   ├── splitter.rs       # Allocation orchestration & waterfill
│   │   ├── plan.rs           # RoutePlan data structures
│   │   └── router.rs         # Main routing entry point
│   ├── adapters/
│   │   ├── cpmm.rs           # Constant Product AMM
│   │   ├── clmm.rs           # Concentrated Liquidity AMM
│   │   ├── clob.rs           # Order Book implementation
│   │   └── curve.rs          # Curve-style stable swap adapter
│   └── sim/
│       └── demo.rs           # Demo graph builder and scenarios
├── examples/
│   └── demo.rs               # Executable demo with 3 scenarios
├── tests/
│   └── test_router.rs        # Integration tests
└── Cargo.toml
```

## Usage

### Running the Demo

```bash
cargo run --example demo
```

This runs three scenarios demonstrating different pool configurations and their impact on routing decisions.

### Running Tests

```bash
cargo test
```

Integration tests verify:
- Route structure and allocation correctness
- Intermediate token whitelist enforcement
- Maximum paths limit enforcement
- Alternate optimization algorithms (golden-section)

### Using as a Library

```rust
use metis::{
    plan_routes,
    CPMMPool,
    OptimizationAlgorithm,
    RoutingConstraints,
    RoutingGraph,
    SplitConfig,
};

// Create a routing graph
let mut rg = RoutingGraph::new();

// Add pools
rg.add_pool_edge(
    "TokenA",
    "TokenB",
    Box::new(CPMMPool::new(
        "pool1".to_string(),
        10_000.0,  // reserve in
        10_000.0,  // reserve out
        0.003,     // 0.3% fee
    )),
    "A_B_direct",
);

// Configure constraints
let mut constraints = RoutingConstraints::default();
constraints.max_hops = 3;
constraints.max_paths = 4;
constraints.optimization_algorithm = OptimizationAlgorithm::GoldenSection;
constraints.split_config = Some(SplitConfig::default());

// Find optimal routes
let plan = plan_routes(&mut rg, "TokenA", "TokenB", 500.0, Some(constraints));

// Access results
println!("Total input: {}", plan.total_in);
println!("Estimated output: {}", plan.est_total_out);
for route in &plan.routes {
    println!("Route allocation: {}", route.amount_in);
}
```

## Algorithm Details

### Waterfill Algorithm

The core routing algorithm uses a greedy waterfill approach:

1. **Initialization**: Calculate initial marginal rates for all candidate paths
2. **Iteration**:
   - Select the path with the highest marginal rate
   - Allocate a small amount (epsilon) to that path
   - Update virtual pool states
   - Recalculate marginal rates for affected paths
3. **Termination**: Continue until the input amount is fully allocated or no profitable paths remain

This approach ensures capital is allocated where it generates the most output, dynamically adjusting as pool states change.

### Candidate Path Generation

1. **Bellman-Ford**: Finds the theoretically optimal single path
2. **DFS-based Multi-Path Search**: Discovers alternative paths
3. **Direct Edges**: Includes all direct token-to-token connections
4. **Deduplication**: Removes duplicate paths

## Migrated from Python

This Rust implementation is a port of the original Python demo (Metis_python), with the following improvements:

- **Type Safety**: Compile-time guarantees via Rust's type system
- **Performance**: Significantly faster execution
- **Memory Safety**: No garbage collector, deterministic resource management
- **Trait-based Design**: Flexible pool implementations via `PoolLike` trait
- **Zero-cost Abstractions**: Generic programming with no runtime overhead

## Key Differences from Python Version

- Uses `petgraph` instead of `networkx` for graph operations
- Implements `PoolLike` as a trait instead of Python Protocol
- Uses `BinaryHeap` for priority queue in waterfill algorithm
- Manual memory management via ownership and borrowing
- Clone-on-write semantics for pool state simulation

## Dependencies

- `petgraph ^0.6` - Graph data structures and algorithms

## License

See LICENSE file for details.

## Contributing

Contributions welcome! Please ensure tests pass before submitting PRs.

## Future Improvements

- [ ] Implement true Yen's K-Shortest Paths algorithm
- [ ] Add more pool types (stable swaps, curve pools, etc.)
- [ ] Optimize path finding for large graphs
- [ ] Add benchmarks
- [ ] Implement proper CLMM tick-based calculations
- [ ] Add serialization support (serde)
- [ ] Gas cost estimation
- [ ] Multi-threaded path finding
