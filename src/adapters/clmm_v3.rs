use crate::core::pool::{PoolLike, Quote};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A single tick in the CLMM (Concentrated Liquidity Market Maker)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tick {
    pub index: i32,         // tick index
    pub net_liquidity: f64, // net liquidity delta at this tick
    pub sqrt_price: f64,    // sqrt(price) at this tick
}

/// Concentrated Liquidity Market Maker (Uniswap V3 style) with tick-based calculations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CLMMTickBased {
    pub id: String,
    pub token_in: String,
    pub token_out: String,
    pub current_sqrt_price: f64, // current sqrt price
    pub current_tick: i32,       // current tick index
    pub liquidity: f64,          // current active liquidity
    pub fee: f64,                // e.g. 0.003 (0.3%)
    pub ticks: Vec<Tick>,        // sorted by tick index
    pub kind: String,
    pub meta: HashMap<String, String>,
}

impl CLMMTickBased {
    /// Create a new tick-based CLMM pool
    pub fn new(
        id: String,
        token_in: String,
        token_out: String,
        current_sqrt_price: f64,
        fee: f64,
        mut ticks: Vec<Tick>,
    ) -> Self {
        // Sort ticks by index
        ticks.sort_by_key(|t| t.index);

        // Calculate current tick and active liquidity
        let current_tick = Self::sqrt_price_to_tick(current_sqrt_price);
        let liquidity = Self::calculate_active_liquidity(&ticks, current_tick);

        Self {
            id,
            token_in,
            token_out,
            current_sqrt_price,
            current_tick,
            liquidity,
            fee,
            ticks,
            kind: "CLMM_V3".to_string(),
            meta: HashMap::new(),
        }
    }

    /// Convert sqrt_price to tick index (simplified)
    pub fn sqrt_price_to_tick(sqrt_price: f64) -> i32 {
        // tick = log(price) / log(1.0001) = 2 * log(sqrt_price) / log(1.0001)
        (2.0 * sqrt_price.ln() / 1.0001_f64.ln()).round() as i32
    }

    /// Convert tick index to sqrt_price
    pub fn tick_to_sqrt_price(tick: i32) -> f64 {
        // sqrt_price = 1.0001^(tick/2)
        1.0001_f64.powf(tick as f64 / 2.0)
    }

    /// Calculate active liquidity at a given tick
    fn calculate_active_liquidity(ticks: &[Tick], current_tick: i32) -> f64 {
        let mut liquidity = 0.0;
        for tick in ticks {
            if tick.index <= current_tick {
                liquidity += tick.net_liquidity;
            }
        }
        liquidity.max(0.0)
    }

    /// Get the next tick boundary in the direction of the swap
    fn get_next_tick(&self, going_up: bool) -> Option<&Tick> {
        if going_up {
            // Find the first tick with index > current_tick
            self.ticks.iter().find(|t| t.index > self.current_tick)
        } else {
            // Find the last tick with index <= current_tick
            self.ticks
                .iter()
                .rev()
                .find(|t| t.index <= self.current_tick)
        }
    }

    /// Calculate amount out for a given amount in, considering tick boundaries
    fn calculate_swap(&self, amount_in: f64) -> (f64, f64) {
        let eff_in = amount_in * (1.0 - self.fee);
        let mut amount_in_remaining = eff_in;
        let mut amount_out_total = 0.0;
        let mut current_sqrt_price = self.current_sqrt_price;
        let mut current_liquidity = self.liquidity;

        // In a V3 swap, we're trading token_in for token_out
        // This means we're moving the price up (increasing sqrt_price)
        let going_up = true;

        while amount_in_remaining > 0.0 && current_liquidity > 1e-6 {
            // Find the next tick boundary
            let next_tick_opt = if going_up {
                self.ticks
                    .iter()
                    .find(|t| Self::tick_to_sqrt_price(t.index) > current_sqrt_price)
            } else {
                self.ticks
                    .iter()
                    .rev()
                    .find(|t| Self::tick_to_sqrt_price(t.index) < current_sqrt_price)
            };

            // Determine target sqrt_price (next tick boundary or maximum price)
            let target_sqrt_price = if let Some(next_tick) = next_tick_opt {
                Self::tick_to_sqrt_price(next_tick.index)
            } else {
                // No more ticks, use a large price (practically infinity)
                current_sqrt_price * 10.0
            };

            // Calculate maximum amount_in that can be consumed before reaching the next tick
            // Using the formula: Δy = L * Δ(1/√P) for token Y (amount_in)
            // Simplified: amount_in ≈ L * (sqrt_price_target - sqrt_price_current)
            let max_amount_in = current_liquidity * (target_sqrt_price - current_sqrt_price).abs();

            let (amount_in_step, sqrt_price_next) = if amount_in_remaining >= max_amount_in {
                // We'll cross the tick boundary
                (max_amount_in, target_sqrt_price)
            } else {
                // We won't reach the next tick
                let new_sqrt_price = current_sqrt_price + amount_in_remaining / current_liquidity;
                (amount_in_remaining, new_sqrt_price)
            };

            // Calculate amount_out for this step
            // Using: Δx = L * Δ(√P) for token X (amount_out)
            let amount_out_step = current_liquidity * (sqrt_price_next - current_sqrt_price).abs();

            amount_out_total += amount_out_step;
            amount_in_remaining -= amount_in_step;
            current_sqrt_price = sqrt_price_next;

            // If we crossed a tick, update liquidity
            if let Some(next_tick) = next_tick_opt {
                if (sqrt_price_next - Self::tick_to_sqrt_price(next_tick.index)).abs() < 1e-9 {
                    current_liquidity += next_tick.net_liquidity;
                    if current_liquidity < 1e-6 {
                        break; // No more liquidity available
                    }
                }
            } else {
                break; // No more ticks
            }
        }

        (amount_out_total, current_sqrt_price)
    }
}

impl PoolLike for CLMMTickBased {
    fn id(&self) -> &str {
        &self.id
    }

    fn kind(&self) -> &str {
        &self.kind
    }

    fn meta(&self) -> &HashMap<String, String> {
        &self.meta
    }

    fn quote(&self, amount_in: f64) -> Quote {
        let (amount_out, _final_sqrt_price) = self.calculate_swap(amount_in);
        let rate = if amount_in > 0.0 {
            amount_out / amount_in
        } else {
            0.0
        };

        Quote {
            amount_out,
            effective_rate: rate,
            fee: self.fee,
        }
    }

    fn marginal_rate(&self, _amount_in: f64) -> f64 {
        // Marginal rate at current price with current liquidity
        // For a V3 pool: rate ≈ sqrt_price * (1 - fee)
        // This is a simplified approximation
        if self.liquidity > 1e-6 {
            self.current_sqrt_price * (1.0 - self.fee)
        } else {
            0.0
        }
    }

    fn apply_virtual_fill(&mut self, amount_in: f64) -> f64 {
        let (amount_out, new_sqrt_price) = self.calculate_swap(amount_in);

        // Update the pool state
        self.current_sqrt_price = new_sqrt_price;
        self.current_tick = Self::sqrt_price_to_tick(new_sqrt_price);
        self.liquidity = Self::calculate_active_liquidity(&self.ticks, self.current_tick);

        amount_out
    }

    fn clone_box(&self) -> Box<dyn PoolLike> {
        Box::new(self.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tick_to_sqrt_price() {
        let sqrt_price = CLMMTickBased::tick_to_sqrt_price(0);
        assert!((sqrt_price - 1.0).abs() < 1e-6);

        let sqrt_price_100 = CLMMTickBased::tick_to_sqrt_price(100);
        assert!(sqrt_price_100 > 1.0);
    }

    #[test]
    fn test_clmm_v3_quote() {
        let ticks = vec![
            Tick {
                index: -100,
                net_liquidity: 50000.0,
                sqrt_price: 0.995,
            },
            Tick {
                index: 0,
                net_liquidity: 100000.0,
                sqrt_price: 1.0,
            },
            Tick {
                index: 100,
                net_liquidity: -50000.0,
                sqrt_price: 1.005,
            },
        ];

        let pool = CLMMTickBased::new(
            "test_v3".to_string(),
            "APT".to_string(),
            "USDT".to_string(),
            1.0,
            0.003,
            ticks,
        );

        let quote = pool.quote(1000.0);
        assert!(quote.amount_out > 0.0);
        assert!(quote.effective_rate > 0.0);
    }
}
