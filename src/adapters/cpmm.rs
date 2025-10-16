use crate::core::pool::{PoolLike, Quote};
use std::collections::HashMap;

/// Constant Product Market Maker pool
#[derive(Debug, Clone)]
pub struct CPMMPool {
    pub id: String,
    pub x: f64,          // reserve_in
    pub y: f64,          // reserve_out
    pub fee: f64,        // e.g. 0.003
    pub kind: String,
    pub meta: HashMap<String, String>,
}

impl CPMMPool {
    pub fn new(id: String, x: f64, y: f64, fee: f64) -> Self {
        Self {
            id,
            x,
            y,
            fee,
            kind: "CPMM".to_string(),
            meta: HashMap::new(),
        }
    }
}

impl PoolLike for CPMMPool {
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
        let eff_in = amount_in * (1.0 - self.fee);
        let out = if eff_in > 0.0 {
            (self.y * eff_in) / (self.x + eff_in)
        } else {
            0.0
        };
        let rate = if amount_in > 0.0 {
            out / amount_in
        } else {
            0.0
        };
        Quote {
            amount_out: out,
            effective_rate: rate,
            fee: self.fee,
        }
    }

    fn marginal_rate(&self, amount_in: f64) -> f64 {
        // μ(δ) = y*x/(x+(1-fee)δ)^2 * (1-fee)
        let t = (1.0 - self.fee) * amount_in;
        let denom = self.x + t;
        (self.y * self.x / (denom * denom)) * (1.0 - self.fee)
    }

    fn apply_virtual_fill(&mut self, amount_in: f64) -> f64 {
        let q = self.quote(amount_in);
        let eff_in = amount_in * (1.0 - self.fee);
        self.x += eff_in;
        self.y -= q.amount_out;
        q.amount_out
    }

    fn clone_box(&self) -> Box<dyn PoolLike> {
        Box::new(self.clone())
    }
}
