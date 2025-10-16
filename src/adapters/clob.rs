use crate::core::pool::{PoolLike, Quote};
use std::collections::HashMap;

/// Central Limit Order Book (Top N levels)
#[derive(Debug, Clone)]
pub struct CLOBTopN {
    pub id: String,
    pub steps: Vec<(f64, f64)>, // [(out_per_in, max_out_remaining)]
    pub fee: f64,
    pub kind: String,
    pub meta: HashMap<String, String>,
    pub idx: usize, // current level pointer
}

impl CLOBTopN {
    pub fn new(id: String, steps: Vec<(f64, f64)>, fee: f64) -> Self {
        Self {
            id,
            steps,
            fee,
            kind: "CLOB".to_string(),
            meta: HashMap::new(),
            idx: 0,
        }
    }

    fn current(&self) -> (f64, f64) {
        if self.idx < self.steps.len() {
            self.steps[self.idx]
        } else {
            (0.0, 0.0)
        }
    }
}

impl PoolLike for CLOBTopN {
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
        let mut remain = amount_in;
        let mut out = 0.0;
        let mut i = self.idx;

        while remain > 0.0 && i < self.steps.len() {
            let (price, size_rem) = self.steps[i];
            if price <= 0.0 || size_rem <= 0.0 {
                i += 1;
                continue;
            }
            let cap_in = size_rem / price;
            let take_in = remain.min(cap_in);
            out += take_in * price;
            remain -= take_in;
            if take_in >= cap_in {
                i += 1;
            }
        }

        out *= 1.0 - self.fee;
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

    fn marginal_rate(&self, _amount_in: f64) -> f64 {
        let (price, size_rem) = self.current();
        if size_rem > 0.0 {
            (1.0 - self.fee) * price
        } else {
            0.0
        }
    }

    fn apply_virtual_fill(&mut self, amount_in: f64) -> f64 {
        let mut remain = amount_in;
        let mut out = 0.0;

        while remain > 0.0 && self.idx < self.steps.len() {
            let (price, size_rem) = self.steps[self.idx];
            if price <= 0.0 || size_rem <= 0.0 {
                self.idx += 1;
                continue;
            }
            let cap_in = size_rem / price;
            let take_in = remain.min(cap_in);
            out += take_in * price;
            self.steps[self.idx].1 = size_rem - take_in * price;
            remain -= take_in;
            if self.steps[self.idx].1 <= 1e-12 {
                self.idx += 1;
            }
        }

        out *= 1.0 - self.fee;
        out
    }

    fn clone_box(&self) -> Box<dyn PoolLike> {
        Box::new(self.clone())
    }
}
