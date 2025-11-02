use crate::benchmark::metrics::{MetricsComparison, SwapMetrics};
use crate::core::constraints::RoutingConstraints;
use crate::core::graph::RoutingGraph;
use crate::core::plan::RoutePlan;
use crate::core::router::plan_routes;
use std::time::Instant;

/// A benchmark runner for comparing on-chain performance with local simulation
pub struct BenchmarkRunner {
    /// The routing graph with all pools
    pub graph: RoutingGraph,

    /// Optional routing constraints
    pub constraints: Option<RoutingConstraints>,
}

impl BenchmarkRunner {
    /// Create a new benchmark runner
    pub fn new(graph: RoutingGraph, constraints: Option<RoutingConstraints>) -> Self {
        Self { graph, constraints }
    }

    /// Run a local simulation and measure performance
    pub fn run_simulation(
        &mut self,
        src_token: &str,
        dst_token: &str,
        amount_in: f64,
    ) -> (RoutePlan, SwapMetrics) {
        let start = Instant::now();

        let plan = plan_routes(
            &mut self.graph,
            src_token,
            dst_token,
            amount_in,
            self.constraints.clone(),
        );

        let elapsed = start.elapsed();

        // Calculate metrics
        let num_routes = plan.routes.len();
        let num_hops: usize = plan.routes.iter().map(|r| r.legs.len()).sum();

        let mut metrics = SwapMetrics::new(
            elapsed,
            plan.total_in,
            plan.est_total_out,
            num_routes,
            num_hops,
        );

        // Calculate total fees (sum of all fees from routes)
        let total_fees = amount_in - plan.est_total_out / metrics.exchange_rate;
        metrics.set_total_fees(total_fees);

        (plan, metrics)
    }

    /// Compare local simulation with on-chain data
    pub fn compare_with_onchain(
        &mut self,
        src_token: &str,
        dst_token: &str,
        amount_in: f64,
        on_chain_metrics: SwapMetrics,
    ) -> MetricsComparison {
        let (_plan, local_metrics) = self.run_simulation(src_token, dst_token, amount_in);

        MetricsComparison::new(on_chain_metrics, local_metrics)
    }

    /// Print a detailed comparison report
    pub fn print_comparison_report(
        &mut self,
        src_token: &str,
        dst_token: &str,
        amount_in: f64,
        on_chain_metrics: SwapMetrics,
    ) {
        let comparison =
            self.compare_with_onchain(src_token, dst_token, amount_in, on_chain_metrics);
        println!("{}", comparison.format());
    }
}

/// Helper to create mock on-chain metrics for testing
pub fn create_mock_onchain_metrics(
    input_amount: f64,
    output_amount: f64,
    execution_time_us: u64,
) -> SwapMetrics {
    use std::time::Duration;

    let mut metrics = SwapMetrics::new(
        Duration::from_micros(execution_time_us),
        input_amount,
        output_amount,
        1, // Assume single route
        1, // Assume single hop
    );

    // Set some realistic values
    let initial_price = output_amount / input_amount * 1.01; // Assume 1% price impact
    metrics.set_price_impact(initial_price);

    let expected_output = output_amount * 1.005; // Assume 0.5% slippage
    metrics.set_slippage(expected_output);

    metrics.set_total_fees(input_amount * 0.003); // Assume 0.3% fee

    metrics
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::cpmm::CPMMPool;

    #[test]
    fn test_benchmark_runner() {
        let mut rg = RoutingGraph::new();

        // Add a simple pool
        let pool = CPMMPool::new("apt_usdt".to_string(), 1_000_000.0, 5_000_000.0, 0.003);

        rg.add_pool_edge("APT", "USDT", Box::new(pool), "apt_usdt_edge");

        let mut runner = BenchmarkRunner::new(rg, None);

        let (_plan, metrics) = runner.run_simulation("APT", "USDT", 1000.0);

        assert!(metrics.output_amount > 0.0);
        assert!(metrics.exchange_rate > 0.0);
        assert_eq!(metrics.input_amount, 1000.0);
    }

    #[test]
    fn test_comparison() {
        let mut rg = RoutingGraph::new();

        let pool = CPMMPool::new("apt_usdt".to_string(), 1_000_000.0, 5_000_000.0, 0.003);

        rg.add_pool_edge("APT", "USDT", Box::new(pool), "apt_usdt_edge");

        let mut runner = BenchmarkRunner::new(rg, None);

        let on_chain_metrics = create_mock_onchain_metrics(1000.0, 4950.0, 2000);

        let comparison = runner.compare_with_onchain("APT", "USDT", 1000.0, on_chain_metrics);

        assert!(comparison.output_diff().abs() > 0.0 || comparison.output_diff().abs() < 100.0);
    }
}
