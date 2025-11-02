use crate::adapters::clmm_v3::{CLMMTickBased, Tick};
use crate::adapters::clob::CLOBTopN;
use crate::adapters::cpmm::CPMMPool;
use crate::adapters::curve::CurveStablePool;
use crate::core::pool::PoolLike;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// JSON configuration for a V2 (CPMM) pool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct V2PoolConfig {
    pub id: String,
    pub token_in: String,
    pub token_out: String,
    pub reserve_in: f64,
    pub reserve_out: f64,
    pub fee: f64,
}

/// JSON configuration for a V3 (CLMM) tick
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TickConfig {
    pub index: i32,
    pub net_liquidity: f64,
}

/// JSON configuration for a V3 (CLMM) pool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct V3PoolConfig {
    pub id: String,
    pub token_in: String,
    pub token_out: String,
    pub current_sqrt_price: f64,
    pub fee: f64,
    pub ticks: Vec<TickConfig>,
}

/// JSON configuration for a CLOB pool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CLOBPoolConfig {
    pub id: String,
    pub token_in: String,
    pub token_out: String,
    pub fee: f64,
    pub levels: Vec<(f64, f64)>, // [(price, size)]
}

/// JSON configuration for a Curve-style stable swap pool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurvePoolConfig {
    pub id: String,
    pub tokens: Vec<String>,
    pub balances: Vec<f64>,
    pub amplification: f64,
    pub fee: f64,
    pub admin_fee: f64,
    #[serde(default)]
    pub token_in: Option<String>,
    #[serde(default)]
    pub token_out: Option<String>,
}

/// Unified pool configuration enum
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum PoolConfig {
    V2(V2PoolConfig),
    V3(V3PoolConfig),
    CLOB(CLOBPoolConfig),
    Curve(CurvePoolConfig),
}

/// Root configuration structure for loading pools from JSON
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolsConfig {
    pub pools: Vec<PoolConfig>,
}

/// Pool factory for creating pool instances from configuration
pub struct PoolFactory;

impl PoolFactory {
    /// Create a pool from a configuration
    pub fn create_pool(config: PoolConfig) -> Result<Box<dyn PoolLike>, String> {
        match config {
            PoolConfig::V2(v2_config) => {
                let pool = CPMMPool::new(
                    v2_config.id,
                    v2_config.reserve_in,
                    v2_config.reserve_out,
                    v2_config.fee,
                );
                Ok(Box::new(pool))
            }
            PoolConfig::V3(v3_config) => {
                // Convert TickConfig to Tick
                let ticks: Vec<Tick> = v3_config
                    .ticks
                    .iter()
                    .map(|tc| {
                        let sqrt_price = CLMMTickBased::tick_to_sqrt_price(tc.index);
                        Tick {
                            index: tc.index,
                            net_liquidity: tc.net_liquidity,
                            sqrt_price,
                        }
                    })
                    .collect();

                let pool = CLMMTickBased::new(
                    v3_config.id,
                    v3_config.token_in,
                    v3_config.token_out,
                    v3_config.current_sqrt_price,
                    v3_config.fee,
                    ticks,
                );
                Ok(Box::new(pool))
            }
            PoolConfig::CLOB(clob_config) => {
                let pool = CLOBTopN::new(clob_config.id, clob_config.levels, clob_config.fee);
                Ok(Box::new(pool))
            }
            PoolConfig::Curve(curve_config) => {
                let token_in = curve_config
                    .token_in
                    .clone()
                    .or_else(|| curve_config.tokens.first().cloned())
                    .ok_or_else(|| "Curve pool missing token_in".to_string())?;
                let token_out = curve_config
                    .token_out
                    .clone()
                    .or_else(|| curve_config.tokens.get(1).cloned())
                    .ok_or_else(|| "Curve pool missing token_out".to_string())?;

                let pool = CurveStablePool::new(
                    curve_config.id,
                    curve_config.tokens,
                    curve_config.balances,
                    curve_config.amplification,
                    curve_config.fee,
                    curve_config.admin_fee,
                    token_in,
                    token_out,
                )
                .map_err(|e| format!("Failed to create Curve pool: {}", e))?;
                Ok(Box::new(pool))
            }
        }
    }

    /// Load pools from a JSON file
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Vec<Box<dyn PoolLike>>, String> {
        let content =
            fs::read_to_string(path).map_err(|e| format!("Failed to read file: {}", e))?;

        let config: PoolsConfig =
            serde_json::from_str(&content).map_err(|e| format!("Failed to parse JSON: {}", e))?;

        let mut pools = Vec::new();
        for pool_config in config.pools {
            let pool = Self::create_pool(pool_config)?;
            pools.push(pool);
        }

        Ok(pools)
    }

    /// Load pools from a JSON string
    pub fn load_from_json(json: &str) -> Result<Vec<Box<dyn PoolLike>>, String> {
        let config: PoolsConfig =
            serde_json::from_str(json).map_err(|e| format!("Failed to parse JSON: {}", e))?;

        let mut pools = Vec::new();
        for pool_config in config.pools {
            let pool = Self::create_pool(pool_config)?;
            pools.push(pool);
        }

        Ok(pools)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_from_json() {
        let json = r#"{
            "pools": [
                {
                    "type": "V2",
                    "id": "apt_usdt_v2",
                    "token_in": "APT",
                    "token_out": "USDT",
                    "reserve_in": 1000000.0,
                    "reserve_out": 5000000.0,
                    "fee": 0.003
                },
                {
                    "type": "V3",
                    "id": "apt_usdt_v3",
                    "token_in": "APT",
                    "token_out": "USDT",
                    "current_sqrt_price": 2.236,
                    "fee": 0.003,
                    "ticks": [
                        {"index": -100, "net_liquidity": 50000.0},
                        {"index": 0, "net_liquidity": 100000.0},
                        {"index": 100, "net_liquidity": -50000.0}
                    ]
                },
                {
                    "type": "Curve",
                    "id": "usdc_usdt_curve",
                    "tokens": ["USDC", "USDT"],
                    "balances": [10000000.0, 10050000.0],
                    "amplification": 100.0,
                    "fee": 0.0004,
                    "admin_fee": 0.5,
                    "token_in": "USDC",
                    "token_out": "USDT"
                }
            ]
        }"#;

        let pools = PoolFactory::load_from_json(json).unwrap();
        assert_eq!(pools.len(), 3);
        assert_eq!(pools[0].kind(), "CPMM");
        assert_eq!(pools[1].kind(), "CLMM_V3");
        assert_eq!(pools[2].kind(), "CurveStable");
    }
}
