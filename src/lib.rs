pub mod adapters;
pub mod benchmark;
pub mod core;
pub mod loader;
pub mod sim;

// Re-export commonly used types
pub use core::constraints::{OptimizationAlgorithm, RoutingConstraints, SplitConfig};
pub use core::graph::RoutingGraph;
pub use core::plan::{RouteAlloc, RouteLeg, RoutePlan};
pub use core::pool::{Edge, PoolLike, Quote};
pub use core::router::plan_routes;

pub use adapters::clmm::CLMMSimpleApprox;
pub use adapters::clmm_v3::{CLMMTickBased, Tick};
pub use adapters::clob::CLOBTopN;
pub use adapters::cpmm::CPMMPool;
pub use adapters::curve::CurveStablePool;

pub use loader::pool_loader::{PoolConfig, PoolFactory, PoolsConfig};

pub use benchmark::comparison::BenchmarkRunner;
pub use benchmark::metrics::{MetricsComparison, SwapMetrics};
