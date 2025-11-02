use crate::core::pool::{PoolLike, Quote};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const MAX_ITER: usize = 256;
const CONVERGENCE_TOL: f64 = 1e-9;

/// Curve-like stable swap pool supporting 2-8 assets with amplification coefficient.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurveStablePool {
    pub id: String,
    pub tokens: Vec<String>,
    pub balances: Vec<f64>,
    pub amplification: f64,
    pub fee: f64,
    pub admin_fee: f64,
    pub token_in: String,
    pub token_out: String,
    token_in_idx: usize,
    token_out_idx: usize,
    kind: String,
    meta: HashMap<String, String>,
}

impl CurveStablePool {
    pub fn new(
        id: String,
        tokens: Vec<String>,
        balances: Vec<f64>,
        amplification: f64,
        fee: f64,
        admin_fee: f64,
        token_in: String,
        token_out: String,
    ) -> Result<Self, String> {
        if tokens.len() < 2 {
            return Err("Curve pool requires at least two tokens".to_string());
        }
        if tokens.len() != balances.len() {
            return Err("Token list and balances length mismatch".to_string());
        }
        if amplification <= 0.0 {
            return Err("Amplification must be positive".to_string());
        }
        let token_in_idx = tokens
            .iter()
            .position(|t| t == &token_in)
            .ok_or_else(|| format!("token_in {} not found in pool", token_in))?;
        let token_out_idx = tokens
            .iter()
            .position(|t| t == &token_out)
            .ok_or_else(|| format!("token_out {} not found in pool", token_out))?;
        if token_in_idx == token_out_idx {
            return Err("token_in and token_out must differ".to_string());
        }

        let mut meta = HashMap::new();
        meta.insert("pool_type".to_string(), "CurveStable".to_string());
        meta.insert("amplification".to_string(), amplification.to_string());
        meta.insert("fee".to_string(), fee.to_string());
        meta.insert("admin_fee".to_string(), admin_fee.to_string());
        meta.insert("tokens".to_string(), tokens.join(","));
        meta.insert("token_in".to_string(), token_in.clone());
        meta.insert("token_out".to_string(), token_out.clone());

        Ok(Self {
            id,
            tokens,
            balances,
            amplification,
            fee,
            admin_fee,
            token_in,
            token_out,
            token_in_idx,
            token_out_idx,
            kind: "CurveStable".to_string(),
            meta,
        })
    }

    fn calculate_d(&self, balances: &[f64]) -> f64 {
        let n = balances.len();
        let mut sum = 0.0;
        for &x in balances {
            sum += x;
        }
        if sum <= 0.0 {
            return 0.0;
        }

        let ann = self.amplification * n as f64;
        let mut d = sum;
        for _ in 0..MAX_ITER {
            let mut d_prod = d;
            for &x in balances {
                if x <= 0.0 {
                    return 0.0;
                }
                d_prod = d_prod * d / (x * n as f64);
            }
            let d_prev = d;
            let numerator = (ann * sum + d_prod * n as f64) * d;
            let denominator = (ann - 1.0) * d + (n as f64 + 1.0) * d_prod;
            d = numerator / denominator;
            if (d - d_prev).abs() <= CONVERGENCE_TOL {
                break;
            }
        }
        d
    }

    fn get_y(&self, i: usize, j: usize, x: f64, balances: &[f64], d: f64) -> f64 {
        let n = balances.len();
        let ann = self.amplification * n as f64;

        let mut c = d;
        let mut sum = 0.0;
        for (idx, &bal) in balances.iter().enumerate() {
            let cur = if idx == i {
                x
            } else if idx == j {
                continue;
            } else {
                bal
            };
            sum += cur;
            c = c * d / (cur * n as f64);
        }
        c = c * d / (ann * n as f64);
        let b = sum + d / ann;
        let mut y = d;
        for _ in 0..MAX_ITER {
            let y_prev = y;
            let numerator = y * y + c;
            let denominator = 2.0 * y + b - d;
            if denominator.abs() <= CONVERGENCE_TOL {
                break;
            }
            y = numerator / denominator;
            if (y - y_prev).abs() <= CONVERGENCE_TOL {
                break;
            }
        }
        y
    }

    fn apply_swap(&mut self, amount_in: f64) -> f64 {
        if amount_in <= 0.0 {
            return 0.0;
        }
        let in_idx = self.token_in_idx;
        let out_idx = self.token_out_idx;
        let mut xp = self.balances.clone();
        let amount_in_effective = amount_in * (1.0 - self.fee);
        xp[in_idx] += amount_in_effective;

        let d = self.calculate_d(&self.balances);
        let y = self.get_y(in_idx, out_idx, xp[in_idx], &xp, d);
        let dy = (self.balances[out_idx] - y).max(0.0);
        let fee_cut = dy * self.fee * self.admin_fee;
        let amount_out = (dy - fee_cut).max(0.0);

        self.balances[in_idx] = xp[in_idx];
        self.balances[out_idx] = (y + fee_cut).max(0.0);
        amount_out
    }
}

impl PoolLike for CurveStablePool {
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
        if amount_in <= 0.0 {
            return Quote {
                amount_out: 0.0,
                effective_rate: 0.0,
                fee: self.fee,
            };
        }

        let mut cloned = self.clone();
        let amount_out = cloned.apply_swap(amount_in);
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
        let tiny = 1e-6;
        let q = self.quote(tiny);
        if tiny > 0.0 {
            q.amount_out / tiny
        } else {
            0.0
        }
    }

    fn apply_virtual_fill(&mut self, amount_in: f64) -> f64 {
        self.apply_swap(amount_in)
    }

    fn clone_box(&self) -> Box<dyn PoolLike> {
        Box::new(self.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_pool() -> CurveStablePool {
        CurveStablePool::new(
            "curve_usdc_usdt".to_string(),
            vec!["USDC".to_string(), "USDT".to_string()],
            vec![10_000_000.0, 10_000_000.0],
            100.0,
            0.0004,
            0.5,
            "USDC".to_string(),
            "USDT".to_string(),
        )
        .unwrap()
    }

    #[test]
    fn quote_is_close_to_parity() {
        let pool = sample_pool();
        let quote = pool.quote(1_000.0);
        assert!(quote.amount_out > 990.0);
        assert!(quote.amount_out < 1_010.0);
    }

    #[test]
    fn apply_virtual_fill_updates_balances() {
        let mut pool = sample_pool();
        let out = pool.apply_virtual_fill(5000.0);
        assert!(out > 0.0);
        assert!(pool.balances[pool.token_in_idx] > 10_000_000.0);
        assert!(pool.balances[pool.token_out_idx] < 10_000_000.0);
    }
}
