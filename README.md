# Metis - DeFi Routing Optimizer

A Rust implementation of a sophisticated DeFi routing optimizer that finds optimal trading paths across multiple liquidity sources using the waterfill algorithm.

## Overview

Metis is a routing engine that optimizes token swaps across heterogeneous liquidity pools (CPMM, CLMM, CLOB) by:
- Finding multiple candidate paths using graph algorithms (Bellman-Ford, DFS-based K-shortest paths)
- Allocating capital across paths using a marginal-rate-based waterfill algorithm
- Supporting complex multi-hop routes with configurable constraints

## Features

- **Multiple Pool Types**:
  - CPMM (Constant Product Market Maker) - e.g., Uniswap V2
  - CLMM (Concentrated Liquidity Market Maker) - e.g., Uniswap V3
  - CLOB (Central Limit Order Book) - order book with multiple price levels

- **Advanced Routing**:
  - Bellman-Ford algorithm for optimal path finding
  - K-shortest paths for candidate generation
  - Waterfill algorithm for optimal capital allocation across routes
  - Virtual fill simulation to account for pool state changes

- **Flexible Configuration**:
  - Maximum hops per path
  - Maximum number of concurrent paths
  - Intermediate token whitelisting
  - Slippage tolerance control
  - Step size for marginal rate calculation

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
│   │   ├── splitter.rs       # Waterfill capital allocation
│   │   ├── plan.rs           # RoutePlan data structures
│   │   └── router.rs         # Main routing entry point
│   ├── adapters/
│   │   ├── cpmm.rs           # Constant Product AMM
│   │   ├── clmm.rs           # Concentrated Liquidity AMM
│   │   └── clob.rs           # Order Book implementation
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

All 3 integration tests verify:
- Route structure and allocation correctness
- Intermediate token whitelist enforcement
- Maximum paths limit enforcement

### Using as a Library

```rust
use metis::{RoutingGraph, CPMMPool, plan_routes, RoutingConstraints};

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
