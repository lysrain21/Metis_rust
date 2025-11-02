use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Performance metrics for a swap execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwapMetrics {
    /// Execution time in microseconds
    pub execution_time_us: u64,

    /// Exchange rate (output / input)
    pub exchange_rate: f64,

    /// Price impact in basis points (bps)
    pub price_impact_bps: f64,

    /// Slippage in basis points (bps)
    pub slippage_bps: f64,

    /// Total fees paid
    pub total_fees: f64,

    /// Output amount
    pub output_amount: f64,

    /// Input amount
    pub input_amount: f64,

    /// Number of routes used
    pub num_routes: usize,

    /// Number of hops (pool swaps) total
    pub num_hops: usize,
}

impl SwapMetrics {
    /// Create a new SwapMetrics with basic info
    pub fn new(
        execution_time: Duration,
        input_amount: f64,
        output_amount: f64,
        num_routes: usize,
        num_hops: usize,
    ) -> Self {
        let exchange_rate = if input_amount > 0.0 {
            output_amount / input_amount
        } else {
            0.0
        };

        Self {
            execution_time_us: execution_time.as_micros() as u64,
            exchange_rate,
            price_impact_bps: 0.0, // Will be calculated later
            slippage_bps: 0.0,     // Will be calculated later
            total_fees: 0.0,       // Will be calculated later
            output_amount,
            input_amount,
            num_routes,
            num_hops,
        }
    }

    /// Calculate price impact given an initial price
    pub fn set_price_impact(&mut self, initial_price: f64) {
        if initial_price > 0.0 {
            let price_change = ((self.exchange_rate - initial_price) / initial_price).abs();
            self.price_impact_bps = price_change * 10000.0;
        }
    }

    /// Set slippage given an expected output
    pub fn set_slippage(&mut self, expected_output: f64) {
        if expected_output > 0.0 {
            let slippage = ((expected_output - self.output_amount) / expected_output).abs();
            self.slippage_bps = slippage * 10000.0;
        }
    }

    /// Set total fees
    pub fn set_total_fees(&mut self, total_fees: f64) {
        self.total_fees = total_fees;
    }

    /// Format metrics as a readable string
    pub fn format(&self) -> String {
        format!(
            "SwapMetrics:\n\
             ├─ Input:           {:.4}\n\
             ├─ Output:          {:.4}\n\
             ├─ Exchange Rate:   {:.6}\n\
             ├─ Execution Time:  {} μs\n\
             ├─ Routes:          {}\n\
             ├─ Hops:            {}\n\
             ├─ Total Fees:      {:.6}\n\
             ├─ Price Impact:    {:.2} bps\n\
             └─ Slippage:        {:.2} bps",
            self.input_amount,
            self.output_amount,
            self.exchange_rate,
            self.execution_time_us,
            self.num_routes,
            self.num_hops,
            self.total_fees,
            self.price_impact_bps,
            self.slippage_bps,
        )
    }
}

/// Comparison between two swap metrics (e.g., on-chain vs local simulation)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsComparison {
    pub on_chain: SwapMetrics,
    pub local_sim: SwapMetrics,
}

impl MetricsComparison {
    /// Create a new comparison
    pub fn new(on_chain: SwapMetrics, local_sim: SwapMetrics) -> Self {
        Self {
            on_chain,
            local_sim,
        }
    }

    /// Calculate the difference in exchange rate (bps)
    pub fn exchange_rate_diff_bps(&self) -> f64 {
        let diff = ((self.local_sim.exchange_rate - self.on_chain.exchange_rate)
            / self.on_chain.exchange_rate)
            .abs();
        diff * 10000.0
    }

    /// Calculate the difference in output amount
    pub fn output_diff(&self) -> f64 {
        self.local_sim.output_amount - self.on_chain.output_amount
    }

    /// Calculate the difference in output amount (percentage)
    pub fn output_diff_pct(&self) -> f64 {
        if self.on_chain.output_amount > 0.0 {
            ((self.local_sim.output_amount - self.on_chain.output_amount)
                / self.on_chain.output_amount)
                * 100.0
        } else {
            0.0
        }
    }

    /// Format comparison as a readable string
    pub fn format(&self) -> String {
        format!(
            "\n╔═══════════════════════════════════════════════════════════════╗\n\
             ║                   SWAP METRICS COMPARISON                     ║\n\
             ╠═══════════════════════════════════════════════════════════════╣\n\
             ║ Metric              │  On-Chain         │  Local Sim        ║\n\
             ╟─────────────────────┼───────────────────┼───────────────────╢\n\
             ║ Input Amount        │ {:>16.4} │ {:>16.4} ║\n\
             ║ Output Amount       │ {:>16.4} │ {:>16.4} ║\n\
             ║ Exchange Rate       │ {:>16.6} │ {:>16.6} ║\n\
             ║ Execution Time (μs) │ {:>16} │ {:>16} ║\n\
             ║ Routes              │ {:>16} │ {:>16} ║\n\
             ║ Hops                │ {:>16} │ {:>16} ║\n\
             ║ Total Fees          │ {:>16.6} │ {:>16.6} ║\n\
             ║ Price Impact (bps)  │ {:>16.2} │ {:>16.2} ║\n\
             ║ Slippage (bps)      │ {:>16.2} │ {:>16.2} ║\n\
             ╟─────────────────────┴───────────────────┴───────────────────╢\n\
             ║ DIFFERENCES:                                                  ║\n\
             ║ ├─ Output Diff:      {:>16.4} ({:>+6.2}%)              ║\n\
             ║ └─ Rate Diff:        {:>16.2} bps                        ║\n\
             ╚═══════════════════════════════════════════════════════════════╝",
            self.on_chain.input_amount,
            self.local_sim.input_amount,
            self.on_chain.output_amount,
            self.local_sim.output_amount,
            self.on_chain.exchange_rate,
            self.local_sim.exchange_rate,
            self.on_chain.execution_time_us,
            self.local_sim.execution_time_us,
            self.on_chain.num_routes,
            self.local_sim.num_routes,
            self.on_chain.num_hops,
            self.local_sim.num_hops,
            self.on_chain.total_fees,
            self.local_sim.total_fees,
            self.on_chain.price_impact_bps,
            self.local_sim.price_impact_bps,
            self.on_chain.slippage_bps,
            self.local_sim.slippage_bps,
            self.output_diff(),
            self.output_diff_pct(),
            self.exchange_rate_diff_bps(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_swap_metrics() {
        let mut metrics = SwapMetrics::new(Duration::from_micros(1500), 1000.0, 5000.0, 2, 3);

        assert_eq!(metrics.exchange_rate, 5.0);
        assert_eq!(metrics.execution_time_us, 1500);

        metrics.set_price_impact(4.8);
        assert!(metrics.price_impact_bps > 0.0);

        metrics.set_slippage(5100.0);
        assert!(metrics.slippage_bps > 0.0);
    }

    #[test]
    fn test_metrics_comparison() {
        let on_chain = SwapMetrics::new(Duration::from_micros(2000), 1000.0, 4950.0, 2, 3);

        let local_sim = SwapMetrics::new(Duration::from_micros(1500), 1000.0, 5000.0, 2, 3);

        let comparison = MetricsComparison::new(on_chain, local_sim);

        assert!(comparison.output_diff() > 0.0);
        assert!(comparison.output_diff_pct() > 0.0);
    }
}
