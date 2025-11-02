# Metis Rust - 使用指南

## 概述

Metis Rust 是一个模块化的 DeFi 路由优化器，支持多种池子类型，并提供性能测试框架来对比链上实际表现和本地模拟。

## 新功能特性

### 1. 模块化池子系统

现在支持从 JSON 配置文件一键加载各种类型的池子：

- **V2 池子 (CPMM)**: 恒定乘积做市商（如 Uniswap V2）
- **V3 池子 (CLMM)**: 基于 tick 的集中流动性做市商（如 Uniswap V3）
- **CLOB 池子**: 中央限价订单簿

### 2. 真正的 V3 实现

完整实现了基于 tick 的 Uniswap V3 风格池子：
- 支持每个 tick 的 net liquidity
- 跨 tick 边界的价格计算
- 流动性聚合和状态更新

### 3. 性能测试框架

提供完整的性能对比工具：
- **速度指标**: 算法执行时间、兑换率
- **磨损指标**: 价格滑点、价格影响、手续费损耗
- **对比报告**: 链上数据 vs 本地模拟的详细对比

## 快速开始

### 1. 准备池子配置文件

创建 JSON 配置文件（例如 `pools.json`）：

```json
{
  "pools": [
    {
      "type": "V2",
      "id": "pancakeswap_apt_usdt",
      "token_in": "APT",
      "token_out": "USDT",
      "reserve_in": 1000000.0,
      "reserve_out": 5000000.0,
      "fee": 0.0025
    },
    {
      "type": "V3",
      "id": "uniswap_v3_apt_usdt",
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
      "type": "CLOB",
      "id": "orderbook_apt_usdt",
      "token_in": "APT",
      "token_out": "USDT",
      "fee": 0.001,
      "levels": [
        [5.02, 10000.0],
        [5.01, 15000.0],
        [5.00, 20000.0]
      ]
    }
  ]
}
```

### 2. 加载池子并构建路由图

```rust
use metis::{PoolFactory, RoutingGraph};

// 从 JSON 文件加载池子
let pools = PoolFactory::load_from_file("pools.json")
    .expect("Failed to load pools");

// 构建路由图
let mut graph = RoutingGraph::new();
for (i, pool) in pools.into_iter().enumerate() {
    graph.add_pool_edge("APT", "USDT", pool, &format!("edge_{}", i));
}
```

### 3. 运行路由优化

```rust
use metis::{plan_routes, OptimizationAlgorithm, RoutingConstraints, SplitConfig};

// 设置约束条件
let constraints = RoutingConstraints {
    max_hops: 3,
    max_paths: 4,
    candidate_pool_size: 10,
    optimization_algorithm: OptimizationAlgorithm::GoldenSection,
    split_config: Some(SplitConfig::default()),
    ..Default::default()
};

// 执行路由规划
let plan = plan_routes(
    &mut graph,
    "APT",
    "USDT",
    10000.0,
    Some(constraints)
);

println!("Total output: {}", plan.est_total_out);
```

### 4. 性能测试和对比

```rust
use metis::{BenchmarkRunner, create_mock_onchain_metrics};

// 创建 benchmark runner
let mut runner = BenchmarkRunner::new(graph, Some(constraints));

// 模拟链上数据（实际使用中从区块链获取）
let on_chain_metrics = create_mock_onchain_metrics(
    10000.0,  // input amount
    49500.0,  // on-chain output
    25000     // execution time (μs)
);

// 运行对比并打印报告
runner.print_comparison_report(
    "APT",
    "USDT",
    10000.0,
    on_chain_metrics
);
```

## 运行示例

我们提供了一个完整的示例程序：

```bash
cargo run --example benchmark_apt_usdt
```

输出将包括：
- 池子加载信息
- 路由图构建过程
- 性能对比报告（链上 vs 本地）
- 详细的路由分析

## JSON 配置格式说明

### V2 池子配置

```json
{
  "type": "V2",
  "id": "pool_unique_id",
  "token_in": "TOKEN_A",
  "token_out": "TOKEN_B",
  "reserve_in": 1000000.0,
  "reserve_out": 5000000.0,
  "fee": 0.003
}
```

字段说明：
- `reserve_in`: token_in 的储备量
- `reserve_out`: token_out 的储备量
- `fee`: 手续费率（如 0.003 = 0.3%）

### V3 池子配置

```json
{
  "type": "V3",
  "id": "pool_unique_id",
  "token_in": "TOKEN_A",
  "token_out": "TOKEN_B",
  "current_sqrt_price": 2.236,
  "fee": 0.003,
  "ticks": [
    {"index": -100, "net_liquidity": 50000.0},
    {"index": 0, "net_liquidity": 100000.0},
    {"index": 100, "net_liquidity": -50000.0}
  ]
}
```

字段说明：
- `current_sqrt_price`: 当前的 sqrt(price)
- `ticks`: tick 数组，每个 tick 包含：
  - `index`: tick 索引
  - `net_liquidity`: 该 tick 的净流动性变化

### CLOB 池子配置

```json
{
  "type": "CLOB",
  "id": "pool_unique_id",
  "token_in": "TOKEN_A",
  "token_out": "TOKEN_B",
  "fee": 0.001,
  "levels": [
    [5.02, 10000.0],
    [5.01, 15000.0],
    [5.00, 20000.0]
  ]
}
```

字段说明：
- `levels`: 订单簿价格层级，每层为 `[price, size]`

## 性能指标说明

### 速度指标

- **算法执行时间** (`execution_time_us`): 路由计算耗时（微秒）
- **兑换率** (`exchange_rate`): 输出/输入的比率

### 磨损指标

- **价格滑点** (`slippage_bps`): 预期价格与实际价格的差异（基点）
- **价格影响** (`price_impact_bps`): 交易对池子价格的影响程度（基点）
- **手续费损耗** (`total_fees`): 所有池子收取的总手续费

## 测试

运行所有测试：

```bash
cargo test
```

测试包括：
- 单元测试（各模块功能测试）
- 集成测试（完整路由流程测试）
- V3 池子 tick 计算测试
- JSON 加载功能测试
- 性能测试框架测试

## 架构说明

```
src/
├── adapters/           # 池子适配器
│   ├── cpmm.rs        # V2 恒定乘积池
│   ├── clmm_v3.rs     # V3 集中流动性池（基于tick）
│   └── clob.rs        # 订单簿池
├── benchmark/         # 性能测试框架
│   ├── metrics.rs     # 性能指标定义
│   └── comparison.rs  # 对比测试工具
├── loader/            # 配置加载器
│   └── pool_loader.rs # JSON 池子加载
└── core/              # 核心路由算法
    ├── router.rs      # 主路由器
    ├── graph.rs       # 路由图
    ├── splitter.rs    # Waterfill 分配算法
    └── ...
```

## 下一步

1. **链上数据集成**: 从实际区块链获取池子状态
2. **更多池子类型**: 添加 Curve、Balancer 等池子支持
3. **Gas 估算**: 添加交易成本估算
4. **并行优化**: 多线程路径查找
5. **实时更新**: 支持池子状态实时更新

## 参考资料

- [Uniswap V2 文档](https://docs.uniswap.org/protocol/V2/introduction)
- [Uniswap V3 文档](https://docs.uniswap.org/protocol/concepts/V3-overview/concentrated-liquidity)
- [Metis 算法论文](https://github.com/your-repo/metis-paper)
