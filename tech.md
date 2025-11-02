<img src="https://r2cdn.perplexity.ai/pplx-full-logo-primary-dark%402x.png" style="height:64px;margin-right:32px"/>

# Solana上的聚合器 Jupiter 的Metis算法是如何做的，根据最新消息，写一个非常详细的给技术人员的技术文档，注意他的每一个路由应该都是经过不同的池子吧，那么每类池子都有设计一个接口方便快速接入模拟数据，比如类uniswap的池子， v2 的话就是 reserve 数据，v3 的话就是每个 tick 分段当前的 net liquditiy 作为模拟。

Now I'll create the comprehensive technical documentation based on all the research gathered.

# Jupiter Metis 算法技术文档

## 概述

Jupiter 是 Solana 区块链上领先的去中心化交易聚合器（DEX Aggregator），其核心路由算法经历了从 Metis 到 Iris 的演进。本文档详细阐述 Jupiter 路由算法的技术实现，特别关注其如何通过标准化接口集成不同类型的流动性池，并为技术人员提供模拟数据接口的设计方案。

## 1. 算法核心原理

### 1.1 Metis 算法基础

Metis 是 Jupiter v3 引入的路由算法，基于 Bellman-Ford 算法的深度改进版本。其核心设计目标是在 Solana 的超高速区块时间内，实现大规模动态环境下的最优价格路由。[^1][^2]

**核心特性：**

- **增量路由构建（Incremental Route Building）**：通过流式处理输入代币，逐步构建可在任意阶段分割和合并的路由。通过迭代生成每个分割的路由，算法允许在不同分割中使用相同的 DEX，从而为复杂交易找到更优价格。[^2][^1]
- **路由生成与报价融合**：将路由生成和报价计算合并为单一步骤，避免生成和使用劣质路由，这不仅提升效率，还允许使用更大的代币集作为中间代币。[^1][^2]
- **Solana 约束适配**：v2 版本在 DEX 数量较少时运行快速，因为 Solana 将单次交换限制为最多 4 个 DEX（由于 64 个账户锁的限制）。Metis 设计为可扩展，当账户锁限制放宽时能包含更多 DEX，并且仅需适度增加运行时间就能支持更多 DEX。[^2][^1]


### 1.2 Iris 路由引擎（2025年最新）

2025年10月，Jupiter 推出了 Ultra v3，引入了新一代路由引擎 Iris，取代了 Metis 系统。[^3][^4][^5][^6]

**Iris 的关键改进：**

- **元聚合（Meta Aggregation）**：Iris 整合来自多个路由源的报价，包括 Jupiter 自己的 Iris 路由、JupiterZ（RFQ 系统）、DFlow、Hashflow 和 OKX。[^7][^5][^3]
- **性能提升 100 倍**：采用高级数学优化算法，如黄金分割法（Golden-section）和布伦特方法（Brent's method），实现路由性能提升 100 倍。[^4][^5][^7]
- **精确分割**：支持精确到 0.01% 的订单分割，使用复杂的优化算法确保最佳执行。[^8][^4]
- **实时自适应**：通过 Predictive Execution（预测执行）功能，基于实际链上条件而非理论报价智能优先处理路由。[^5][^9][^7]

**架构对比：**


| 特性 | Metis (v3, 2023) | Iris (Ultra v3, 2025) |
| :-- | :-- | :-- |
| 基础算法 | 改进的 Bellman-Ford | 黄金分割法 + 布伦特方法 |
| 路由源 | 单一聚合 | 元聚合（多路由源） |
| 性能提升 | 比 v2 优 5.22% | 比 Metis 优 100 倍 |
| 分割精度 | 标准分割 | 0.01% 精度分割 |
| DEX 限制 | 4 个 DEX/交易 | 动态扩展 |
| RFQ 集成 | 无 | JupiterZ 原生集成 |

## 2. DEX 集成架构

### 2.1 AMM 接口标准

Jupiter 通过标准化的 AMM（Automated Market Maker）接口集成各类 DEX。所有 DEX 必须实现以下 Rust trait：[^10]

```rust
pub trait Amm {
    // 从 keyed account 创建 AMM 实例
    fn from_keyed_account(keyed_account: &KeyedAccount, amm_context: &AmmContext) -> Result<Self>
    where
        Self: Sized;

    // 返回底层 DEX 的可读标签
    fn label(&self) -> String;
    
    // 返回程序 ID
    fn program_id(&self) -> Pubkey;
    
    // 返回池状态或市场状态地址
    fn key(&self) -> Pubkey;
    
    // 返回可交易的代币 mints
    fn get_reserve_mints(&self) -> Vec<Pubkey>;
    
    // 返回生成报价所需的账户
    fn get_accounts_to_update(&self) -> Vec<Pubkey>;
    
    // 更新内部状态（执行重度反序列化和预计算缓存）
    fn update(&mut self, account_map: &AccountMap) -> Result<()>;
    
    // 生成报价
    fn quote(&self, quote_params: &QuoteParams) -> Result<Quote>;
    
    // 返回执行交换所需的交换指令和账户元数据
    fn get_swap_and_account_metas(&self, swap_params: &SwapParams) -> Result<SwapAndAccountMetas>;
    
    // 指示 get_accounts_to_update 是否可能返回非常量 vec
    fn has_dynamic_accounts(&self) -> bool {
        false
    }
    
    // 指示在调用 get_reserve_mints 前是否需要调用 update
    fn requires_update_for_reserve_mints(&self) -> bool {
        false
    }
    
    // 指示是否支持 ExactOut 模式
    fn supports_exact_out(&self) -> bool {
        false
    }
    
    fn get_user_setup(&self) -> Option<AmmUserSetup> {
        None
    }
    
    fn clone_amm(&self) -> Box<dyn Amm + Send + Sync>;
    
    // 仅能单向交易（从第一个 mint 到第二个 mint）
    fn unidirectional(&self) -> bool {
        false
    }
    
    // 测试用途：依赖程序映射
    fn program_dependencies(&self) -> Vec<(Pubkey, String)> {
        vec![]
    }
    
    fn get_accounts_len(&self) -> usize {
        32 // 默认接近完整 legacy 交易以惩罚未实现
    }
    
    // 底层流动性标识符
    fn underlying_liquidities(&self) -> Option<HashSet<Pubkey>> {
        None
    }
    
    // 提供快捷方式确定 AMM 是否可用于交易
    fn is_active(&self) -> bool {
        true
    }
}
```

**关键约束：**

- **无网络调用**：`get_accounts_to_update` 提供需要获取的账户，这些账户由 Jupiter 核心引擎批量缓存并通过 `update` 传递给 AMM 实例。可能对 `quote` 进行多次调用使用同一缓存，因此整个实现中**不允许任何网络调用**。[^10]


### 2.2 不同池类型的接口设计

#### 2.2.1 Uniswap V2 类型池（恒定乘积 CPAMM）

**原理：**

Uniswap V2 使用恒定乘积公式 \$ x \times y = k \$，其中 \$ x \$ 和 \$ y \$ 是两种代币的储备量。[^11]

**模拟数据接口：**

```rust
pub struct UniswapV2Pool {
    pub pool_address: Pubkey,
    pub token_a_mint: Pubkey,
    pub token_b_mint: Pubkey,
    pub reserve_a: u64,      // 代币 A 的储备量
    pub reserve_b: u64,      // 代币 B 的储备量
    pub fee_bps: u16,        // 费用（基点），通常为 30 (0.3%)
}

impl UniswapV2Pool {
    // 根据恒定乘积公式计算输出
    pub fn get_amount_out(&self, amount_in: u64, reserve_in: u64, reserve_out: u64) -> u64 {
        let amount_in_with_fee = amount_in as u128 * (10000 - self.fee_bps as u128);
        let numerator = amount_in_with_fee * reserve_out as u128;
        let denominator = (reserve_in as u128 * 10000) + amount_in_with_fee;
        (numerator / denominator) as u64
    }
    
    // 实现 quote 功能
    pub fn quote(&self, amount_in: u64, input_mint: &Pubkey) -> Result<u64> {
        let (reserve_in, reserve_out) = if input_mint == &self.token_a_mint {
            (self.reserve_a, self.reserve_b)
        } else {
            (self.reserve_b, self.reserve_a)
        };
        
        Ok(self.get_amount_out(amount_in, reserve_in, reserve_out))
    }
}

// 模拟数据结构
#[derive(Clone, Debug)]
pub struct V2PoolSimulationData {
    pub reserve_a: u64,
    pub reserve_b: u64,
    pub timestamp: i64,
}

impl V2PoolSimulationData {
    // 从链上数据创建
    pub fn from_account_data(data: &[u8]) -> Result<Self> {
        // 解析账户数据获取储备量
        // Solana 账户数据通常使用 Borsh 序列化
        Ok(Self {
            reserve_a: u64::from_le_bytes(data[0..8].try_into()?),
            reserve_b: u64::from_le_bytes(data[8..16].try_into()?),
            timestamp: i64::from_le_bytes(data[16..24].try_into()?),
        })
    }
}
```


#### 2.2.2 Uniswap V3 类型池（集中流动性 CLAMM）

**原理：**

Uniswap V3 引入集中流动性（Concentrated Liquidity），允许 LP 在特定价格区间内提供流动性。每个价格区间（tick）都有其自己的流动性量。[^12][^13][^14]

**关键概念：**

- **Tick（价格点）**：预定义的价格位置，流动性提供者不能在任意价格边界提供流动性，而必须在预定义的 tick 处。[^13]
- **Net Liquidity（净流动性）**：每个 tick 的净流动性变化，当价格跨越 tick 时，Uniswap V3 重新计算可用流动性。[^14][^13]
- **虚拟储备**：在每个价格区间内，池的行为类似于 Uniswap V2，但 \$ k \$ 值（流动性）不同。[^12][^14]

**模拟数据接口：**

```rust
pub struct UniswapV3Pool {
    pub pool_address: Pubkey,
    pub token_a_mint: Pubkey,
    pub token_b_mint: Pubkey,
    pub current_tick: i32,
    pub current_sqrt_price: u128,    // sqrt(price) 以 Q64.64 定点数表示
    pub liquidity: u128,              // 当前活跃流动性
    pub fee_tier: u32,                // 费用等级（如 500, 3000, 10000 表示 0.05%, 0.3%, 1%）
    pub tick_spacing: i32,            // tick 间距
    pub ticks: BTreeMap<i32, TickInfo>,  // tick -> 流动性信息
}

#[derive(Clone, Debug)]
pub struct TickInfo {
    pub liquidity_net: i128,      // 该 tick 的净流动性变化
    pub liquidity_gross: u128,    // 该 tick 的总流动性
    pub initialized: bool,        // tick 是否已初始化
}

impl UniswapV3Pool {
    // 计算跨越 tick 时的输出
    pub fn quote(&self, amount_in: u64, zero_for_one: bool) -> Result<QuoteResult> {
        let mut amount_remaining = amount_in as u128;
        let mut amount_out = 0u128;
        let mut current_sqrt_price = self.current_sqrt_price;
        let mut current_liquidity = self.liquidity;
        let mut current_tick = self.current_tick;
        
        while amount_remaining > 0 {
            // 找到下一个已初始化的 tick
            let next_tick = self.get_next_initialized_tick(current_tick, zero_for_one)?;
            let target_sqrt_price = self.tick_to_sqrt_price(next_tick);
            
            // 在当前 tick 范围内计算交换
            let (sqrt_price_next, amount_in_step, amount_out_step) = 
                self.compute_swap_step(
                    current_sqrt_price,
                    target_sqrt_price,
                    current_liquidity,
                    amount_remaining,
                    self.fee_tier,
                )?;
            
            amount_remaining -= amount_in_step;
            amount_out += amount_out_step;
            
            // 如果达到 tick 边界，更新流动性
            if sqrt_price_next == target_sqrt_price {
                if let Some(tick_info) = self.ticks.get(&next_tick) {
                    // 跨越 tick 时更新流动性
                    current_liquidity = if zero_for_one {
                        current_liquidity.wrapping_sub(tick_info.liquidity_net as u128)
                    } else {
                        current_liquidity.wrapping_add(tick_info.liquidity_net as u128)
                    };
                }
                current_tick = if zero_for_one { next_tick - 1 } else { next_tick };
            }
            
            current_sqrt_price = sqrt_price_next;
        }
        
        Ok(QuoteResult {
            amount_out: amount_out as u64,
            final_sqrt_price: current_sqrt_price,
            final_tick: current_tick,
        })
    }
    
    // 在单个 tick 范围内计算交换步骤
    fn compute_swap_step(
        &self,
        sqrt_price_current: u128,
        sqrt_price_target: u128,
        liquidity: u128,
        amount_remaining: u128,
        fee_tier: u32,
    ) -> Result<(u128, u128, u128)> {
        // 实现 Uniswap V3 的交换数学
        // 参考: https://uniswap.org/whitepaper-v3.pdf
        
        let zero_for_one = sqrt_price_current >= sqrt_price_target;
        let exact_in = true; // 假设精确输入
        
        // 简化实现（实际需要更复杂的数学）
        let sqrt_price_next = if exact_in {
            // 计算下一个价格
            self.get_next_sqrt_price_from_input(
                sqrt_price_current,
                liquidity,
                amount_remaining,
                zero_for_one,
            )?
        } else {
            sqrt_price_target
        };
        
        // 计算输入和输出金额
        let (amount_in, amount_out) = self.calculate_amounts(
            sqrt_price_current,
            sqrt_price_next,
            liquidity,
            zero_for_one,
        )?;
        
        // 应用费用
        let fee_amount = (amount_in * fee_tier as u128) / 1_000_000;
        let amount_in_with_fee = amount_in + fee_amount;
        
        Ok((sqrt_price_next, amount_in_with_fee, amount_out))
    }
    
    fn get_next_initialized_tick(&self, tick: i32, zero_for_one: bool) -> Result<i32> {
        if zero_for_one {
            // 向下寻找下一个已初始化的 tick
            self.ticks.range(..tick)
                .rev()
                .find(|(_, info)| info.initialized)
                .map(|(t, _)| *t)
                .ok_or(Error::NoTickFound)
        } else {
            // 向上寻找下一个已初始化的 tick
            self.ticks.range(tick..)
                .find(|(_, info)| info.initialized)
                .map(|(t, _)| *t)
                .ok_or(Error::NoTickFound)
        }
    }
    
    fn tick_to_sqrt_price(&self, tick: i32) -> u128 {
        // 将 tick 转换为 sqrt(price)
        // sqrt(price) = 1.0001^(tick/2)
        // 实现使用定点数数学
        let abs_tick = tick.abs() as u128;
        let mut ratio = if tick < 0 {
            0xfffcb933bd6fad37aa2d162d1a594001u128
        } else {
            0x100000000000000000000000000000000u128
        };
        
        // 位操作优化的幂运算
        // ... (省略详细实现)
        
        ratio
    }
}

// 模拟数据结构
#[derive(Clone, Debug)]
pub struct V3PoolSimulationData {
    pub current_tick: i32,
    pub current_sqrt_price: u128,
    pub liquidity: u128,
    pub fee_tier: u32,
    pub tick_data: Vec<TickSimulationData>,
}

#[derive(Clone, Debug)]
pub struct TickSimulationData {
    pub tick_index: i32,
    pub liquidity_net: i128,
    pub liquidity_gross: u128,
    pub fee_growth_outside_0: u128,
    pub fee_growth_outside_1: u128,
}

impl V3PoolSimulationData {
    // 从链上数据批量获取 tick 信息
    pub async fn fetch_tick_range(
        &self,
        tick_lower: i32,
        tick_upper: i32,
        tick_spacing: i32,
    ) -> Result<Vec<TickSimulationData>> {
        let mut ticks = Vec::new();
        let mut current_tick = tick_lower;
        
        while current_tick <= tick_upper {
            // 从链上获取 tick 数据
            // 在实际实现中，这会是批量 RPC 调用
            ticks.push(TickSimulationData {
                tick_index: current_tick,
                liquidity_net: 0,
                liquidity_gross: 0,
                fee_growth_outside_0: 0,
                fee_growth_outside_1: 0,
            });
            
            current_tick += tick_spacing;
        }
        
        Ok(ticks)
    }
}
```


#### 2.2.3 Curve 类型池（稳定币 AMM）

**原理：**

Curve 使用 StableSwap 不变量，专为类似资产（如稳定币）设计，提供极低的滑点。[^12]

**模拟数据接口：**

```rust
pub struct CurvePool {
    pub pool_address: Pubkey,
    pub tokens: Vec<Pubkey>,
    pub balances: Vec<u64>,
    pub amplification_coefficient: u64,  // A 参数
    pub fee_bps: u16,
}

impl CurvePool {
    // Curve StableSwap 不变量: A * n^n * sum(x_i) + D = A * D * n^n + D^(n+1) / (n^n * prod(x_i))
    pub fn quote(&self, amount_in: u64, input_index: usize, output_index: usize) -> Result<u64> {
        let n = self.balances.len();
        let a = self.amplification_coefficient;
        
        // 计算 D (池的总价值不变量)
        let d = self.calculate_d(&self.balances, a, n)?;
        
        // 更新输入余额
        let mut new_balances = self.balances.clone();
        new_balances[input_index] += amount_in;
        
        // 计算新的输出余额
        let new_y = self.get_y(input_index, output_index, new_balances[input_index], &new_balances, a, d)?;
        
        let amount_out = self.balances[output_index] - new_y;
        let fee = (amount_out * self.fee_bps as u64) / 10000;
        
        Ok(amount_out - fee)
    }
    
    fn calculate_d(&self, balances: &[u64], a: u64, n: usize) -> Result<u64> {
        // 迭代计算 D
        let sum: u64 = balances.iter().sum();
        if sum == 0 {
            return Ok(0);
        }
        
        let mut d = sum;
        let ann = a * (n as u64).pow(n as u32);
        
        for _ in 0..256 {  // 最大迭代次数
            let mut d_p = d;
            for balance in balances {
                d_p = d_p * d / (balance * n as u64);
            }
            
            let d_prev = d;
            d = (ann * sum + d_p * n as u64) * d / ((ann - 1) * d + (n as u64 + 1) * d_p);
            
            if d > d_prev {
                if d - d_prev <= 1 {
                    break;
                }
            } else if d_prev - d <= 1 {
                break;
            }
        }
        
        Ok(d)
    }
    
    fn get_y(&self, i: usize, j: usize, x: u64, balances: &[u64], a: u64, d: u64) -> Result<u64> {
        // 求解 y (输出代币的新余额)
        let n = balances.len();
        let ann = a * (n as u64).pow(n as u32);
        
        let mut c = d;
        let mut s = 0u64;
        
        for (k, &balance) in balances.iter().enumerate() {
            if k == i {
                s += x;
                c = c * d / (x * n as u64);
            } else if k != j {
                s += balance;
                c = c * d / (balance * n as u64);
            }
        }
        
        c = c * d / (ann * n as u64);
        let b = s + d / ann;
        
        let mut y = d;
        for _ in 0..256 {
            let y_prev = y;
            y = (y * y + c) / (2 * y + b - d);
            
            if y > y_prev {
                if y - y_prev <= 1 {
                    break;
                }
            } else if y_prev - y <= 1 {
                break;
            }
        }
        
        Ok(y)
    }
}

// 模拟数据结构
#[derive(Clone, Debug)]
pub struct CurvePoolSimulationData {
    pub balances: Vec<u64>,
    pub amplification_coefficient: u64,
    pub virtual_price: u64,
    pub admin_fee: u16,
}
```


## 3. 路由算法实现细节

### 3.1 增量路由构建流程

```rust
pub struct MetisRouter {
    pub dexes: Vec<Box<dyn Amm + Send + Sync>>,
    pub max_splits: usize,
    pub max_intermediate_tokens: usize,
}

impl MetisRouter {
    // 主路由函数
    pub fn route(
        &self,
        input_mint: Pubkey,
        output_mint: Pubkey,
        amount: u64,
    ) -> Result<RouteResult> {
        // 1. 初始化路由状态
        let mut best_routes = BTreeMap::new();
        best_routes.insert(input_mint, RouteState {
            amount,
            routes: vec![],
            total_out: 0,
        });
        
        // 2. 增量构建路由（流式处理）
        for split_index in 0..self.max_splits {
            let mut new_routes = BTreeMap::new();
            
            for (current_mint, state) in &best_routes {
                // 为每个当前状态探索所有可能的下一跳
                for dex in &self.dexes {
                    if !dex.is_active() {
                        continue;
                    }
                    
                    let reserve_mints = dex.get_reserve_mints();
                    
                    // 尝试从 current_mint 到其他 mint
                    for next_mint in reserve_mints {
                        if next_mint == *current_mint {
                            continue;
                        }
                        
                        // 在同一个 DEX 上可以有多个分割
                        for split_amount in self.generate_split_amounts(state.amount) {
                            // 获取报价
                            let quote_result = dex.quote(&QuoteParams {
                                input_mint: *current_mint,
                                output_mint: next_mint,
                                amount: split_amount,
                            })?;
                            
                            // 更新或创建新的路由状态
                            let mut new_state = state.clone();
                            new_state.routes.push(RouteHop {
                                dex: dex.label(),
                                input_mint: *current_mint,
                                output_mint: next_mint,
                                amount_in: split_amount,
                                amount_out: quote_result.amount,
                            });
                            
                            // 如果到达目标 mint，计算总输出
                            if next_mint == output_mint {
                                new_state.total_out += quote_result.amount;
                            }
                            
                            // 保留最优路由
                            self.update_best_route(&mut new_routes, next_mint, new_state);
                        }
                    }
                }
            }
            
            // 合并新路由到最优路由集
            best_routes.extend(new_routes);
        }
        
        // 3. 选择最优路由
        best_routes.get(&output_mint)
            .cloned()
            .ok_or(Error::NoRouteFound)
            .map(|state| RouteResult {
                routes: state.routes,
                total_output: state.total_out,
            })
    }
    
    // 生成分割金额
    fn generate_split_amounts(&self, total_amount: u64) -> Vec<u64> {
        let mut splits = vec![total_amount]; // 始终包含全额
        
        // 生成不同比例的分割
        for i in 1..self.max_splits {
            let split_amount = (total_amount * i as u64) / self.max_splits as u64;
            if split_amount > 0 {
                splits.push(split_amount);
            }
        }
        
        splits
    }
    
    fn update_best_route(
        &self,
        routes: &mut BTreeMap<Pubkey, RouteState>,
        mint: Pubkey,
        new_state: RouteState,
    ) {
        routes.entry(mint)
            .and_modify(|existing| {
                if new_state.total_out > existing.total_out {
                    *existing = new_state.clone();
                }
            })
            .or_insert(new_state);
    }
}

#[derive(Clone, Debug)]
pub struct RouteState {
    pub amount: u64,
    pub routes: Vec<RouteHop>,
    pub total_out: u64,
}

#[derive(Clone, Debug)]
pub struct RouteHop {
    pub dex: String,
    pub input_mint: Pubkey,
    pub output_mint: Pubkey,
    pub amount_in: u64,
    pub amount_out: u64,
}

#[derive(Clone, Debug)]
pub struct RouteResult {
    pub routes: Vec<RouteHop>,
    pub total_output: u64,
}
```


### 3.2 并行报价刷新基础设施

```rust
use tokio::sync::RwLock;
use std::sync::Arc;

pub struct QuoteCache {
    cache: Arc<RwLock<HashMap<CacheKey, CachedQuote>>>,
    refresh_interval: Duration,
}

#[derive(Hash, Eq, PartialEq)]
struct CacheKey {
    dex: String,
    input_mint: Pubkey,
    output_mint: Pubkey,
    amount_bucket: u64,  // 金额桶化以提高缓存命中率
}

struct CachedQuote {
    quote: Quote,
    timestamp: Instant,
}

impl QuoteCache {
    // 并行刷新多个池的报价
    pub async fn refresh_quotes_parallel(
        &self,
        dexes: &[Box<dyn Amm + Send + Sync>],
        account_map: &AccountMap,
    ) -> Result<()> {
        let refresh_tasks: Vec<_> = dexes
            .iter()
            .map(|dex| {
                let dex = dex.clone_amm();
                let account_map = account_map.clone();
                
                tokio::spawn(async move {
                    // 更新 DEX 状态
                    let mut dex_mut = dex;
                    dex_mut.update(&account_map)?;
                    
                    // 为常见交易对生成报价
                    // 这些报价会被缓存供后续使用
                    Ok::<_, Error>(())
                })
            })
            .collect();
        
        // 等待所有刷新完成
        for task in refresh_tasks {
            task.await??;
        }
        
        Ok(())
    }
    
    // 批量获取账户数据
    pub async fn batch_fetch_accounts(
        &self,
        rpc_client: &RpcClient,
        dexes: &[Box<dyn Amm + Send + Sync>],
    ) -> Result<AccountMap> {
        let mut all_accounts = HashSet::new();
        
        // 收集所有需要的账户
        for dex in dexes {
            for account in dex.get_accounts_to_update() {
                all_accounts.insert(account);
            }
        }
        
        // 批量获取（Solana 支持单次最多 100 个账户）
        let accounts_vec: Vec<_> = all_accounts.into_iter().collect();
        let mut account_map = HashMap::new();
        
        for chunk in accounts_vec.chunks(100) {
            let accounts = rpc_client
                .get_multiple_accounts(chunk)
                .await?;
            
            for (pubkey, account) in chunk.iter().zip(accounts.iter()) {
                if let Some(acc) = account {
                    account_map.insert(*pubkey, acc.clone());
                }
            }
        }
        
        Ok(account_map)
    }
}
```


### 3.3 Iris 元聚合实现（2025）

```rust
pub struct IrisMetaAggregator {
    pub iris_router: IrisRouter,
    pub jupiterz_rfq: JupiterZRFQ,
    pub dflow_client: DFlowClient,
    pub hashflow_client: HashflowClient,
    pub okx_client: OKXClient,
}

impl IrisMetaAggregator {
    // 从所有源聚合报价
    pub async fn get_best_quote(
        &self,
        input_mint: Pubkey,
        output_mint: Pubkey,
        amount: u64,
    ) -> Result<AggregatedQuote> {
        // 并行从所有源获取报价
        let (iris_quote, rfq_quote, dflow_quote, hashflow_quote, okx_quote) = tokio::join!(
            self.iris_router.get_quote(input_mint, output_mint, amount),
            self.jupiterz_rfq.request_quote(input_mint, output_mint, amount),
            self.dflow_client.get_quote(input_mint, output_mint, amount),
            self.hashflow_client.get_quote(input_mint, output_mint, amount),
            self.okx_client.get_quote(input_mint, output_mint, amount),
        );
        
        // 收集所有有效报价
        let mut quotes = Vec::new();
        if let Ok(q) = iris_quote { quotes.push(q); }
        if let Ok(q) = rfq_quote { quotes.push(q); }
        if let Ok(q) = dflow_quote { quotes.push(q); }
        if let Ok(q) = hashflow_quote { quotes.push(q); }
        if let Ok(q) = okx_quote { quotes.push(q); }
        
        // 选择最优报价
        quotes.into_iter()
            .max_by_key(|q| q.output_amount)
            .ok_or(Error::NoQuotesAvailable)
    }
    
    // 使用高级优化算法进行分割
    pub fn optimize_splits_golden_section(
        &self,
        total_amount: u64,
        num_routes: usize,
    ) -> Vec<u64> {
        // 黄金分割搜索实现
        const PHI: f64 = 1.618033988749895;
        let mut splits = vec![0u64; num_routes];
        
        // 使用黄金分割比率优化分配
        // 详细实现省略，但基本思路是：
        // 1. 使用黄金分割比率迭代搜索最优分割点
        // 2. 对每个潜在分割评估总滑点
        // 3. 选择最小化总成本的分割
        
        splits
    }
    
    // 布伦特方法优化
    pub fn optimize_splits_brent(
        &self,
        total_amount: u64,
        routes: &[RouteCandidate],
    ) -> Vec<u64> {
        // 布伦特方法：结合黄金分割、抛物线插值和二分法
        // 用于在无导数情况下找到函数最小值
        
        // 目标函数：最小化总滑点 + 费用
        let objective = |splits: &[u64]| -> f64 {
            routes.iter()
                .zip(splits.iter())
                .map(|(route, &amount)| {
                    route.calculate_cost(amount)
                })
                .sum()
        };
        
        // 布伦特优化实现（简化）
        // 实际实现需要迭代收敛
        vec![total_amount / routes.len() as u64; routes.len()]
    }
}

#[derive(Clone, Debug)]
pub struct RouteCandidate {
    pub source: QuoteSource,
    pub estimated_output: u64,
    pub estimated_slippage: f64,
    pub fee: u64,
}

impl RouteCandidate {
    fn calculate_cost(&self, amount: u64) -> f64 {
        // 考虑滑点和费用的总成本
        let slippage_cost = (amount as f64) * self.estimated_slippage;
        let fee_cost = self.fee as f64;
        slippage_cost + fee_cost
    }
}

#[derive(Clone, Debug)]
pub enum QuoteSource {
    Iris,
    JupiterZ,
    DFlow,
    Hashflow,
    OKX,
}
```


## 4. 性能优化与约束

### 4.1 Solana 账户锁限制

Solana 的账户锁限制为 64 个账户/交易。每个 DEX 通常需要：[^1][^2]

- 程序账户：1
- 池账户：1-3
- 代币账户：2-4
- 用户账户：2

因此，单次交易理论上最多可包含 4-6 个 DEX，具体取决于 DEX 的复杂度。

**优化策略：**

```rust
pub struct TransactionBuilder {
    pub max_accounts: usize,  // 通常设为 64
}

impl TransactionBuilder {
    // 估算路由所需账户数
    pub fn estimate_accounts(&self, route: &RouteResult) -> usize {
        let mut accounts = HashSet::new();
        
        // 基础账户
        accounts.insert("program_id");
        accounts.insert("user_wallet");
        accounts.insert("input_token_account");
        accounts.insert("output_token_account");
        
        // 每个路由跳跃的账户
        for hop in &route.routes {
            // DEX 程序
            accounts.insert(&format!("dex_{}", hop.dex));
            // 池账户
            accounts.insert(&format!("pool_{}_{}", hop.input_mint, hop.output_mint));
            // 中间代币账户（如果需要）
            if hop.output_mint != route.routes.last().unwrap().output_mint {
                accounts.insert(&format!("intermediate_{}", hop.output_mint));
            }
        }
        
        accounts.len()
    }
    
    // 如果超过限制，分解为多个交易
    pub fn build_transactions(&self, route: &RouteResult) -> Vec<Transaction> {
        if self.estimate_accounts(route) <= self.max_accounts {
            vec![self.build_single_transaction(route)]
        } else {
            self.split_into_multiple_transactions(route)
        }
    }
}
```


### 4.2 实时报价刷新

```rust
use tokio::time::{interval, Duration};

pub struct QuoteRefreshService {
    cache: Arc<QuoteCache>,
    refresh_interval: Duration,
}

impl QuoteRefreshService {
    pub async fn start(&self, dexes: Arc<Vec<Box<dyn Amm + Send + Sync>>>) {
        let mut interval = interval(self.refresh_interval);
        
        loop {
            interval.tick().await;
            
            // 获取最新账户数据
            let account_map = self.cache
                .batch_fetch_accounts(&rpc_client, &dexes)
                .await
                .unwrap_or_default();
            
            // 并行刷新所有报价
            if let Err(e) = self.cache.refresh_quotes_parallel(&dexes, &account_map).await {
                eprintln!("Quote refresh error: {}", e);
            }
        }
    }
}
```


## 5. 完整示例：模拟交换执行

```rust
use solana_sdk::pubkey::Pubkey;

#[tokio::main]
async fn main() -> Result<()> {
    // 1. 初始化路由器
    let mut dexes: Vec<Box<dyn Amm + Send + Sync>> = Vec::new();
    
    // 添加 Uniswap V2 类型池
    let raydium_pool = UniswapV2Pool {
        pool_address: Pubkey::new_unique(),
        token_a_mint: USDC_MINT,
        token_b_mint: SOL_MINT,
        reserve_a: 1_000_000_000_000, // 1M USDC
        reserve_b: 5_000_000_000,     // 5K SOL
        fee_bps: 30,
    };
    dexes.push(Box::new(raydium_pool));
    
    // 添加 Uniswap V3 类型池
    let orca_whirlpool = UniswapV3Pool {
        pool_address: Pubkey::new_unique(),
        token_a_mint: USDC_MINT,
        token_b_mint: SOL_MINT,
        current_tick: 50000,
        current_sqrt_price: 1_000_000_000_000,
        liquidity: 10_000_000_000,
        fee_tier: 3000, // 0.3%
        tick_spacing: 60,
        ticks: load_tick_data()?,
    };
    dexes.push(Box::new(orca_whirlpool));
    
    // 添加 Curve 类型池
    let saber_pool = CurvePool {
        pool_address: Pubkey::new_unique(),
        tokens: vec![USDC_MINT, USDT_MINT],
        balances: vec![10_000_000_000_000, 10_000_000_000_000],
        amplification_coefficient: 100,
        fee_bps: 4,
    };
    dexes.push(Box::new(saber_pool));
    
    // 2. 创建 Metis 路由器
    let router = MetisRouter {
        dexes,
        max_splits: 4,
        max_intermediate_tokens: 5,
    };
    
    // 3. 执行路由
    let route = router.route(
        USDC_MINT,
        SOL_MINT,
        100_000_000, // 100 USDC
    )?;
    
    println!("找到最优路由:");
    println!("总输出: {} SOL", route.total_output);
    println!("路由跳跃:");
    for (i, hop) in route.routes.iter().enumerate() {
        println!("  {}. {} -> {} 通过 {} (输入: {}, 输出: {})",
            i + 1,
            hop.input_mint,
            hop.output_mint,
            hop.dex,
            hop.amount_in,
            hop.amount_out,
        );
    }
    
    // 4. 使用 Iris 元聚合（2025 版本）
    let iris_aggregator = IrisMetaAggregator {
        iris_router: IrisRouter::new(router),
        jupiterz_rfq: JupiterZRFQ::new(),
        dflow_client: DFlowClient::new(),
        hashflow_client: HashflowClient::new(),
        okx_client: OKXClient::new(),
    };
    
    let best_quote = iris_aggregator.get_best_quote(
        USDC_MINT,
        SOL_MINT,
        100_000_000,
    ).await?;
    
    println!("\nIris 最优报价:");
    println!("来源: {:?}", best_quote.source);
    println!("输出: {} SOL", best_quote.output_amount);
    
    Ok(())
}

fn load_tick_data() -> Result<BTreeMap<i32, TickInfo>> {
    // 加载或生成 tick 数据
    let mut ticks = BTreeMap::new();
    
    // 示例：在几个关键 tick 上添加流动性
    ticks.insert(49800, TickInfo {
        liquidity_net: 1_000_000_000,
        liquidity_gross: 1_000_000_000,
        initialized: true,
    });
    
    ticks.insert(50200, TickInfo {
        liquidity_net: -1_000_000_000,
        liquidity_gross: 1_000_000_000,
        initialized: true,
    });
    
    Ok(ticks)
}
```


## 6. 测试与模拟框架

### 6.1 池模拟测试

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_uniswap_v2_quote() {
        let pool = UniswapV2Pool {
            pool_address: Pubkey::new_unique(),
            token_a_mint: Pubkey::new_unique(),
            token_b_mint: Pubkey::new_unique(),
            reserve_a: 1_000_000_000,
            reserve_b: 2_000_000_000,
            fee_bps: 30,
        };
        
        let output = pool.quote(
            100_000_000,
            &pool.token_a_mint,
        ).unwrap();
        
        // 验证恒定乘积不变量
        let k_before = pool.reserve_a as u128 * pool.reserve_b as u128;
        let k_after = (pool.reserve_a + 100_000_000) as u128 
                    * (pool.reserve_b - output) as u128;
        
        // k 应该略微增加（由于费用）
        assert!(k_after >= k_before);
    }
    
    #[test]
    fn test_uniswap_v3_tick_crossing() {
        let mut ticks = BTreeMap::new();
        ticks.insert(100, TickInfo {
            liquidity_net: 1_000_000,
            liquidity_gross: 1_000_000,
            initialized: true,
        });
        ticks.insert(200, TickInfo {
            liquidity_net: -500_000,
            liquidity_gross: 500_000,
            initialized: true,
        });
        
        let pool = UniswapV3Pool {
            pool_address: Pubkey::new_unique(),
            token_a_mint: Pubkey::new_unique(),
            token_b_mint: Pubkey::new_unique(),
            current_tick: 150,
            current_sqrt_price: 1_000_000,
            liquidity: 1_000_000,
            fee_tier: 3000,
            tick_spacing: 60,
            ticks,
        };
        
        let result = pool.quote(1_000_000, true).unwrap();
        
        // 验证 tick 越过时流动性更新
        assert_ne!(result.final_tick, pool.current_tick);
    }
    
    #[test]
    fn test_curve_stable_swap() {
        let pool = CurvePool {
            pool_address: Pubkey::new_unique(),
            tokens: vec![Pubkey::new_unique(), Pubkey::new_unique()],
            balances: vec![1_000_000_000, 1_000_000_000],
            amplification_coefficient: 100,
            fee_bps: 4,
        };
        
        let output = pool.quote(10_000_000, 0, 1).unwrap();
        
        // 对于稳定币，滑点应该非常小
        let slippage = ((10_000_000 - output) as f64 / 10_000_000.0) * 100.0;
        assert!(slippage < 0.1); // 小于 0.1%
    }
}
```


### 6.2 路由性能基准测试

```rust
#[cfg(test)]
mod benchmarks {
    use super::*;
    use criterion::{black_box, criterion_group, criterion_main, Criterion};
    
    fn benchmark_routing(c: &mut Criterion) {
        let dexes = setup_test_dexes(10); // 10 个 DEX
        let router = MetisRouter {
            dexes,
            max_splits: 4,
            max_intermediate_tokens: 5,
        };
        
        c.bench_function("route_simple", |b| {
            b.iter(|| {
                router.route(
                    black_box(USDC_MINT),
                    black_box(SOL_MINT),
                    black_box(100_000_000),
                )
            })
        });
    }
    
    fn benchmark_quote_parallel_refresh(c: &mut Criterion) {
        let cache = QuoteCache::new(Duration::from_secs(1));
        let dexes = setup_test_dexes(50);
        
        c.bench_function("parallel_quote_refresh", |b| {
            b.iter(|| {
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async {
                    cache.refresh_quotes_parallel(&dexes, &AccountMap::new()).await
                })
            })
        });
    }
    
    criterion_group!(benches, benchmark_routing, benchmark_quote_parallel_refresh);
    criterion_main!(benches);
}
```


## 7. 总结与最佳实践

### 7.1 关键技术要点

1. **标准化接口**：通过 AMM trait 抽象不同池类型，允许统一处理[^10]
2. **无网络调用约束**：所有报价计算必须基于预缓存的账户数据[^10]
3. **批量数据获取**：利用 Solana 的 `getMultipleAccounts` RPC 批量获取最多 100 个账户
4. **并行报价刷新**：使用异步并发刷新多个池的状态
5. **智能分割**：使用数学优化算法（如黄金分割法、布伦特方法）实现精确分割[^4][^7]

### 7.2 模拟数据接口设计原则

- **V2 池**：仅需储备量（`reserve_a`, `reserve_b`）
- **V3 池**：需要当前 tick、sqrt_price、活跃流动性，以及关键 tick 范围内的净流动性分布
- **Curve 池**：需要所有代币余额和放大系数
- **更新频率**：根据池类型和交易量，V2/Curve 池可每 1-5 秒更新，V3 池因 tick 数据量大可降低频率或按需更新


### 7.3 性能优化建议

1. **账户缓存**：实现多层缓存（内存缓存 + Redis）
2. **预计算**：预计算常见交易对的报价曲线
3. **负载均衡**：使用多个 RPC 节点分散请求
4. **增量更新**：仅更新变化的 tick（V3 池）
5. **滑点保护**：实现实时滑点估算器（RTSE）[^9][^7]

### 7.4 2025 年最新架构（Ultra v3）

Jupiter Ultra v3 引入的关键改进：[^6][^3][^7][^5]

- **Iris Router**：100 倍性能提升，0.01% 分割精度
- **ShadowLane**：延迟从 1.2 秒降至 0.4 秒
- **Predictive Execution**：基于实际链上条件的智能路由
- **Meta Aggregation**：整合 5+ 个流动性源
- **MEV 保护**：34 倍更强的三明治攻击防护
- **无 Gas 支持**：执行费用降低 8-10 倍


## 参考资料

本文档基于以下来源编写：

- Jupiter 官方技术文档与开发者指南[^3][^2][^1][^10]
- Uniswap v2 和 v3 白皮书[^13][^14][^11][^12]
- Curve StableSwap 白皮书[^12]
- Solana 开发者文档
- 最新 Jupiter Ultra v3 发布公告（2025 年 10 月）[^7][^5][^6][^4]

***

**文档版本**: 1.0
**最后更新**: 2025 年 11 月
**作者**: 基于 Jupiter 公开技术资料整理
**许可**: 仅供技术学习与参考
<span style="display:none">[^100][^101][^102][^103][^104][^105][^106][^107][^108][^109][^110][^111][^112][^113][^114][^115][^116][^117][^118][^119][^120][^121][^122][^123][^124][^125][^126][^127][^128][^129][^130][^131][^132][^133][^134][^135][^136][^137][^138][^139][^140][^15][^16][^17][^18][^19][^20][^21][^22][^23][^24][^25][^26][^27][^28][^29][^30][^31][^32][^33][^34][^35][^36][^37][^38][^39][^40][^41][^42][^43][^44][^45][^46][^47][^48][^49][^50][^51][^52][^53][^54][^55][^56][^57][^58][^59][^60][^61][^62][^63][^64][^65][^66][^67][^68][^69][^70][^71][^72][^73][^74][^75][^76][^77][^78][^79][^80][^81][^82][^83][^84][^85][^86][^87][^88][^89][^90][^91][^92][^93][^94][^95][^96][^97][^98][^99]</span>

<div align="center">⁂</div>

[^1]: https://discuss.jup.ag/t/archived-jupiter-v3-the-metis-routing-algo/21712

[^2]: https://discuss.jup.ag/t/jupiter-metis-routing-optimizing-swaps-on-solana/22152

[^3]: https://dev.jup.ag/docs/routing

[^4]: https://www.ainvest.com/news/jupiter-dex-ultra-v3-iris-router-game-changer-defi-liquidity-trading-efficiency-2510/

[^5]: https://blockonomi.com/jupiter-unveils-ultra-v3-its-most-powerful-solana-trading-engine-yet/

[^6]: https://cryptorank.io/news/feed/1efc6-jupiter-launches-ultra-v3-on-solana

[^7]: https://www.fxstreet.com/cryptocurrencies/news/jupiter-unveils-ultra-v3-trading-engine-to-boost-performance-202510172214

[^8]: https://www.theglobeandmail.com/investing/markets/markets-news/Tipranks/35583739/jupiter-launches-ultra-v3-the-ultimate-trading-engine-for-solana/

[^9]: https://zycrypto.com/ultra-v3-jupiter-unveils-the-ultimate-trading-engine-for-solana/

[^10]: https://dev.jup.ag/docs/routing/dex-integration

[^11]: https://rareskills.io/post/uniswap-v2-price-impact

[^12]: https://www.paradigm.xyz/2021/06/uniswap-v3-the-universal-amm

[^13]: https://rareskills.io/post/uniswap-v3-concentrated-liquidity

[^14]: https://arxiv.org/html/2412.18580v1

[^15]: http://arxiv.org/pdf/2406.05568.pdf

[^16]: http://arxiv.org/pdf/2405.08882.pdf

[^17]: https://arxiv.org/pdf/2309.12640.pdf

[^18]: http://arxiv.org/pdf/2406.07200.pdf

[^19]: https://arxiv.org/pdf/2206.13423.pdf

[^20]: http://arxiv.org/pdf/2308.05008.pdf

[^21]: https://arxiv.org/pdf/2411.11279.pdf

[^22]: https://dl.acm.org/doi/pdf/10.1145/3643916.3644406

[^23]: https://www.binance.com/en/square/post/5729086027842

[^24]: https://forklog.com/en/jupiter-dex-aggregator-launches-ultra-v3-protocol/

[^25]: https://www.binance.com/en/square/post/657239569762

[^26]: https://www.solanavolumebot.org/blog/jupiter-aggregator-volume-bot-maximize-spl-token-visibility

[^27]: https://www.bitstamp.net/learn/cryptocurrency-guide/what-is-jupiter-jup/

[^28]: https://www.kucoin.com/learn/web3/what-is-jupiter-jup-solana-dex-and-how-to-use-it

[^29]: https://www.antiersolutions.com/blogs/solana-dex-development-charting-your-course-to-become-the-next-jupiter/

[^30]: https://www.reddit.com/r/solana/comments/15b4yeo/jupiterexchange_announcing_jupiter_v3/

[^31]: https://www.nansen.ai/post/what-is-jupiter-exchange

[^32]: https://dev.jup.ag

[^33]: https://cryptorank.io/news/feed/25f08-solana-dex-jupiter-launches-ultra-v3-with-100x-faster-routing-34x-stronger-protection

[^34]: https://support.jup.ag/hc/en-us

[^35]: https://www.quicknode.com/guides/solana-development/3rd-party-integrations/jupiter-api-trading-bot

[^36]: https://financefeeds.com/jupiter-aggregator-unveils-ultra-v3-with-major-routing-and-mev-protection-upgrades/

[^37]: https://dev.jup.ag/docs/ultra

[^38]: https://solanacompass.com/learn/breakpoint-23/breakpoint-2023-the-metis-routing-algorithm

[^39]: https://chainwire.org/2025/10/20/jupiter-launches-ultra-v3-the-ultimate-trading-engine-for-solana/

[^40]: https://www.mdpi.com/2073-4433/13/5/731

[^41]: https://ieeexplore.ieee.org/document/10568905/

[^42]: https://saemobilus.sae.org/papers/energy-efficiency-technologies-connected-automated-vehicles-findings-arpa-es-nextcar-program-2024-01-1990

[^43]: https://qtanalytics.in/journals/index.php/IJERR/article/view/5308

[^44]: https://ijcsmc.com/docs/papers/August2023/V12I8202311.pdf

[^45]: https://www.semanticscholar.org/paper/d224bb930e2d4aeb68753c91c6be4844b3f8dfc1

[^46]: https://onlinelibrary.wiley.com/doi/10.1002/cae.22471

[^47]: https://dl.acm.org/doi/10.1145/3263878

[^48]: https://www.semanticscholar.org/paper/72d3fc05975552a073f46eaad26239519b541c8e

[^49]: http://link.springer.com/10.1007/978-3-319-60195-3

[^50]: https://arxiv.org/pdf/2407.05901.pdf

[^51]: https://arxiv.org/pdf/2404.04753.pdf

[^52]: http://arxiv.org/pdf/1401.2491.pdf

[^53]: https://arxiv.org/pdf/2412.00513.pdf

[^54]: http://arxiv.org/pdf/2108.06427.pdf

[^55]: https://arxiv.org/pdf/2501.14234.pdf

[^56]: https://arxiv.org/pdf/2408.17211.pdf

[^57]: https://arxiv.org/pdf/2412.13225.pdf

[^58]: https://www.quicknode.com/docs/solana/jupiter-transactions

[^59]: https://coinfactory.app/en/blog/how-to-create-a-liquidity-pool-on-jupiter

[^60]: https://dev.jup.ag/docs/perps/pool-account

[^61]: https://www.blocmates.com/blocmates-101/what-is-jupiter-jup

[^62]: https://solana.unity-sdk.gg/docs/jupiter

[^63]: https://discuss.jup.ag/t/enabling-private-swaps-on-jupiter/39500

[^64]: https://heybeluga.com/articles/how-trade-jupiter-dex-solana/

[^65]: https://gemwallet.com/learn/jupiter-solana-swap-gem-wallet/

[^66]: https://www.mexc.co/en-NG/news/exchange-news-jupiter-exchange-launches-ultra-v3-next-gen-trading-engine/134095

[^67]: https://www.lbank.com/fa/explore/jupiter-dex-aggregator-jup-coin-solana-defi

[^68]: https://ieeexplore.ieee.org/document/11114626/

[^69]: https://www.semanticscholar.org/paper/9981421bda77122736a09508bda1a551cfea410e

[^70]: https://arxiv.org/pdf/2503.07834.pdf

[^71]: https://cryptoeconomicsystems.pubpub.org/pub/angeris-uniswap-analysis/download/pdf

[^72]: https://arxiv.org/pdf/1911.03380.pdf

[^73]: https://arxiv.org/pdf/2411.08145.pdf

[^74]: https://arxiv.org/pdf/2211.01346.pdf

[^75]: http://arxiv.org/pdf/2406.17094.pdf

[^76]: https://arxiv.org/pdf/2410.19107.pdf

[^77]: http://arxiv.org/pdf/2410.18434.pdf

[^78]: https://arxiv.org/html/2509.05013v1

[^79]: https://def-ai.gitbook.io/defai/liquidity-provision/concentrated-liquidity-clmm

[^80]: https://ethresear.ch/t/grim-forker-checks-and-balances-to-amm-protocol-fees/17565

[^81]: https://github.com/idrees535/Uniswap-V3-Simulator

[^82]: https://rocknblock.io/portfolio/beamswap

[^83]: https://www.ixs.finance/learning-hub/amm-market-simulation-part-3-4

[^84]: https://uniswapv3book.com/milestone_1/calculating-liquidity.html

[^85]: https://arxiv.org/abs/2410.09983

[^86]: https://arxiv.org/html/2404.05803v1

[^87]: https://web3-ethereum-defi.readthedocs.io/api/uniswap_v3/_autosummary_uniswap_v3/eth_defi.uniswap_v3.liquidity.html

[^88]: https://docs.algebra.finance/algebra-integral-documentation/overview/why-concentrated-liquidity-and-modularity-matter

[^89]: https://blog.amberdata.io/developing-and-backtesting-a-liquidity-provider-strategy-on-uniswap-v2

[^90]: https://mixbytes.io/blog/uniswap-v3-ticks-dive-into-concentrated-liquidity

[^91]: https://www.sciencedirect.com/science/article/pii/S2096720924000691

[^92]: https://docs.uniswap.org/contracts/v2/concepts/advanced-topics/research

[^93]: https://app.uniswap.org/whitepaper-v3.pdf

[^94]: https://www.ijraset.com/best-journal/hybrid-algorithm-combining-bellmanford-dijkstra-and-machine-learning-for-dynamic-network-routing

[^95]: https://ijitce.org/index.php/ijitce/article/view/1206

[^96]: https://beei.org/index.php/EEI/article/view/577

[^97]: https://link.springer.com/10.1007/978-94-017-8798-7_19

[^98]: https://www.e3s-conferences.org/10.1051/e3sconf/202340003004

[^99]: https://onlinelibrary.wiley.com/doi/10.1002/dac.6056

[^100]: https://journal.gumrf.ru/jour/article/view/228

[^101]: https://dl.acm.org/doi/10.1145/3625156.3625189

[^102]: https://ijsrem.com/download/optimizing-the-bellman-ford-algorithm-using-gpu-parallelization/

[^103]: https://www.semanticscholar.org/paper/6ca02eeb32e8f5703eb81bd431c9383a80ac352d

[^104]: https://arxiv.org/pdf/2402.10343.pdf

[^105]: http://arxiv.org/pdf/1807.04551.pdf

[^106]: https://arxiv.org/pdf/1508.02759.pdf

[^107]: http://arxiv.org/pdf/1901.08326.pdf

[^108]: https://arxiv.org/pdf/1111.5414.pdf

[^109]: http://arxiv.org/pdf/2410.23383.pdf

[^110]: https://linkinghub.elsevier.com/retrieve/pii/S0968090X18303152

[^111]: https://arxiv.org/abs/1905.01325

[^112]: https://www.baeldung.com/cs/bellman-ford

[^113]: https://www.coingecko.com/learn/what-are-dex-aggregators-in-crypto

[^114]: https://drops.dagstuhl.de/storage/00lipics/lipics-vol282-aft2023/LIPIcs.AFT.2023.24/LIPIcs.AFT.2023.24.pdf

[^115]: https://www.geeksforgeeks.org/dsa/bellman-ford-algorithm-dp-23/

[^116]: https://www.chainup.com/blog/dex-aggregators-institutional-defi/

[^117]: https://bonding-curve-research-group.gitbook.io/bonding-curve-research-group-library/case-studies/cow-protocol/batch-trading-and-function-maximizing-amms

[^118]: https://takeuforward.org/data-structure/bellman-ford-algorithm-g-41/

[^119]: https://www.cube.exchange/what-is/dex-aggregator

[^120]: https://threesigma.xyz/blog/defi/distributed-automated-market-makers-damm

[^121]: https://en.wikipedia.org/wiki/Bellman–Ford_algorithm

[^122]: https://www.antiersolutions.com/blogs/what-are-the-5-technical-secrets-to-successful-dex-aggregator-development/

[^123]: https://blog.uniswap.org/measuring-price-improvement-with-order-flow-auctions

[^124]: https://github.com/topics/bellman-ford-algorithm?l=java\&o=asc\&s=stars

[^125]: https://docs.kyberswap.com/getting-started/foundational-topics/decentralized-finance/dex-aggregator

[^126]: https://arxiv.org/html/2408.02634v1

[^127]: https://towardsdatascience.com/bellman-ford-single-source-shortest-path-algorithm-on-gpu-using-cuda-a358da20144b/

[^128]: https://www.gate.com/learn/articles/exploring-8-major-dex-aggregators-engines-driving-efficiency-and-liquidity-in-the-crypto-market/4465

[^129]: https://arxiv.org/html/2508.08152v1

[^130]: https://stackoverflow.com/questions/41517416/implementation-of-bellman-ford-algorithm-in-python

[^131]: https://www.debutinfotech.com/blog/dex-aggregators-vs-standalone-decentralized-exchanges

[^132]: https://arxiv.org/pdf/2304.02343.pdf

[^133]: https://www.mdpi.com/1424-8220/23/4/1881/pdf?version=1675784773

[^134]: https://arxiv.org/pdf/2312.16874.pdf

[^135]: https://www.mantra-networking.com/juniper-routers-routing-engine-re/

[^136]: https://networktik.com/routing-engine-re-in-juniper/

[^137]: https://99bitcoins.com/news/presales/jupiter-ultra-v3-is-here-3-best-solana-meme-coins-to-buy-now/

[^138]: https://www.juniper.net/documentation/us/en/software/junos/junos-overview/topics/concept/junos-software-architecture.html

[^139]: https://www.reflexivityresearch.com/all-reports/jupiter-overview

[^140]: https://www.mitrade.com/au/insights/news/live-news/article-3-1204066-20251018

