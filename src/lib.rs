pub mod adapters;
pub mod core;
pub mod sim;

// Re-export commonly used types
pub use core::constraints::RoutingConstraints;
pub use core::graph::RoutingGraph;
pub use core::plan::{RouteAlloc, RouteLeg, RoutePlan};
pub use core::pool::{Edge, PoolLike, Quote};
pub use core::router::plan_routes;

pub use adapters::clmm::CLMMSimpleApprox;
pub use adapters::clob::CLOBTopN;
pub use adapters::cpmm::CPMMPool;
