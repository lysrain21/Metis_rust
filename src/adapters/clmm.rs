use crate::core::pool::{PoolLike, Quote};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Concentrated Liquidity Market Maker (simple approximation)
/// MVP: treats the current active range as a local curve approximation (similar to CPMM)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CLMMSimpleApprox {
    pub id: String,
    pub x: f64,
    pub y: f64,
    pub fee: f64,
    pub kind: String,
    pub meta: HashMap<String, String>,
}

impl CLMMSimpleApprox {
    pub fn new(id: String, x: f64, y: f64, fee: f64) -> Self {
        Self {
            id,
            x,
            y,
            fee,
            kind: "CLMM".to_string(),
            meta: HashMap::new(),
        }
    }
}

impl PoolLike for CLMMSimpleApprox {
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
