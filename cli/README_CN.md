# DLMM CLI - 中文文档

DLMM（动态流动性做市商）协议的命令行界面工具。

## 📋 目录

- [简介](#简介)
- [安装](#安装)
- [快速开始](#快速开始)
- [命令概览](#命令概览)
- [常用操作指南](#常用操作指南)
- [高级功能](#高级功能)
- [开发指南](#开发指南)

## 简介

DLMM CLI 是一个功能强大的命令行工具，用于与 Solana 区块链上的 DLMM 协议进行交互。它提供了完整的流动性管理、交易、仓位管理等功能。

### 主要特性

- 🔄 **流动性管理**：创建和管理流动性池
- 💱 **交易功能**：执行各种类型的代币交换
- 📊 **仓位管理**：创建、查看和管理流动性仓位
- 💰 **收益管理**：领取手续费和奖励
- 🔧 **管理功能**：协议参数配置和管理（需要权限）

## 安装

### 环境要求

- Rust 1.76.0 或更高版本
- Solana CLI 工具
- 对于 M1 Mac 用户：使用 x86_64-apple-darwin 目标

```bash
# 安装 Rust（如果尚未安装）
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 设置正确的 Rust 版本
rustup default 1.76.0

# M1 Mac 用户需要添加目标
rustup target add x86_64-apple-darwin
```

### 构建项目

```bash
# 克隆仓库
git clone https://github.com/your-repo/dlmm-sdk.git
cd dlmm-sdk/cli

# 构建项目
cargo build --release

# 运行 CLI
./target/release/cli --help
```

## 快速开始

### 1. 配置钱包和网络

```bash
# 使用默认钱包（~/.config/solana/id.json）连接主网
./cli --provider.cluster mainnet <命令>

# 使用自定义钱包
./cli --provider.wallet /path/to/wallet.json <命令>

# 连接到测试网
./cli --provider.cluster devnet <命令>
```

### 2. 创建流动性池

```bash
# 列出所有可用的预设参数（bin步长）
./cli list-all-bin-step

# 创建新的流动性池（以 BTC/USDC 为例）
./cli initialize-pair2 \
  <preset_parameter_pubkey> \
  <btc_mint> \
  <usdc_mint> \
  30000.0  # 初始价格：1 BTC = 30000 USDC
```

### 3. 添加流动性

```bash
# 首先初始化仓位
./cli initialize-position <lb_pair> <lower_price> <upper_price>

# 添加流动性到仓位
./cli add-liquidity \
  <lb_pair> \
  <position> \
  1000000000 \  # X代币数量（BTC）
  30000000000 \ # Y代币数量（USDC）
  --bin-liquidity-distribution "-1,0.0,0.25 0,0.75,0.75 1,0.25,0.0"
```

## 命令概览

### 流动性池管理

| 命令 | 描述 | 使用场景 |
|-----|------|---------|
| `initialize-pair` | 创建流动性池(v1) | 创建基础流动性池 |
| `initialize-pair2` | 创建流动性池(v2) | 创建带预设参数的流动性池 |
| `show-pair` | 显示池信息 | 查看池的当前状态 |
| `list-all-bin-step` | 列出所有bin步长 | 选择合适的费率层级 |

### 仓位管理

| 命令 | 描述 | 使用场景 |
|-----|------|---------|
| `initialize-position` | 初始化仓位 | 创建新的流动性仓位 |
| `add-liquidity` | 添加流动性 | 向仓位存入代币 |
| `remove-liquidity` | 移除流动性 | 从仓位提取代币 |
| `close-position` | 关闭仓位 | 完全退出并回收租金 |
| `show-position` | 显示仓位信息 | 查看仓位详情 |

### 交易操作

| 命令 | 描述 | 使用场景 |
|-----|------|---------|
| `swap-exact-in` | 精确输入交易 | 指定卖出数量 |
| `swap-exact-out` | 精确输出交易 | 指定买入数量 |
| `swap-with-price-impact` | 限制滑点交易 | 控制价格影响 |

### 收益管理

| 命令 | 描述 | 使用场景 |
|-----|------|---------|
| `claim-fee` | 领取手续费 | 提取累积的交易费 |
| `claim-reward` | 领取奖励 | 提取流动性奖励 |

## 常用操作指南

### 创建并管理流动性池

```bash
# 步骤 1: 查看可用的费率层级
./cli list-all-bin-step

# 步骤 2: 创建流动性池
./cli initialize-pair2 \
  GWEKyW3efP3b8zA3vqPgeJYZZQ1fDLxQxHiLDX1qLBhH \  # 预设参数（25基点费率）
  So11111111111111111111111111111111111111112 \   # SOL
  EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v \ # USDC
  150.0  # 1 SOL = 150 USDC

# 步骤 3: 初始化价格范围的bin数组
./cli initialize-bin-array-with-price-range \
  <lb_pair> \
  100.0 \  # 最低价格
  200.0    # 最高价格

# 步骤 4: 创建仓位
./cli initialize-position-with-price-range \
  <lb_pair> \
  120.0 \  # 仓位最低价格
  180.0    # 仓位最高价格

# 步骤 5: 添加流动性
./cli add-liquidity \
  <lb_pair> \
  <position> \
  10000000000 \  # 10 SOL
  1500000000 \   # 1500 USDC
  --bin-liquidity-distribution "0,1.0,1.0"  # 全部集中在当前价格
```

### 执行交易

```bash
# 用 1 SOL 换 USDC（精确输入）
./cli swap-exact-in \
  <lb_pair> \
  1000000000 \  # 1 SOL (9位小数)
  --swap-for-y  # 换成Y代币（USDC）

# 换取正好 100 USDC（精确输出）
./cli swap-exact-out \
  <lb_pair> \
  100000000 \  # 100 USDC (6位小数)
  --swap-for-y
```

### 管理收益

```bash
# 查看仓位信息（包括未领取的费用）
./cli show-position <position>

# 领取累积的手续费
./cli claim-fee <lb_pair> <position>

# 领取奖励（如果有）
./cli claim-reward <lb_pair> <position> --reward-index 0
```

## 高级功能

### ILM（初始流动性管理）

ILM 功能允许操作员高效地播种初始流动性：

```bash
# 使用曲率算法播种流动性
./cli seed-liquidity-by-operator \
  <lb_pair> \
  100000000000 \  # 基础代币数量
  --curvature 0.5 \  # 曲率参数（0-1）
  --max-retries 5

# 单个bin播种
./cli seed-liquidity-single-bin-by-operator \
  <lb_pair> \
  <position> \
  1000000000  # 数量
```

### 管理员功能

需要管理员权限的操作：

```bash
# 设置池状态
./cli admin set-pair-status <lb_pair> --status true

# 提取协议费用
./cli admin withdraw-protocol-fee <lb_pair> <amount>

# 更新基础费率
./cli admin update-base-fee <lb_pair> 30  # 30基点
```

## 开发指南

### 项目结构

```
cli/
├── src/
│   ├── main.rs           # 主入口
│   ├── args.rs           # 命令行参数定义
│   ├── math.rs           # 数学计算工具
│   └── instructions/     # 指令实现
│       ├── mod.rs        # 模块索引
│       ├── admin/        # 管理员指令
│       ├── ilm/          # ILM指令
│       └── ...           # 其他指令
```

### 添加新指令

1. 在 `src/args.rs` 中添加命令定义：
```rust
pub enum DLMMCommand {
    // ... 现有命令
    YourNewCommand(YourNewParams),
}
```

2. 在 `src/instructions/` 创建实现文件
3. 在 `src/instructions/mod.rs` 导出模块
4. 在 `src/main.rs` 添加命令路由

### 测试

```bash
# 运行所有测试
cargo test

# 运行特定测试
cargo test test_name

# 使用 clippy 检查代码
cargo clippy

# 格式化代码
cargo fmt
```

## 常见问题

### Q: 如何设置优先费用？
A: 使用 `--priority-fee` 参数：
```bash
./cli --priority-fee 10000 swap-exact-in ...
```

### Q: 如何处理 Token2022 代币？
A: CLI 自动检测并处理 Token2022 代币，无需额外配置。

### Q: 如何获取测试代币？
A: 在测试网上，可以使用 Solana 水龙头获取 SOL，然后通过测试 DEX 交换其他代币。

## 贡献

欢迎提交 Issue 和 Pull Request！请确保：
- 代码通过所有测试
- 添加必要的中文注释
- 遵循现有的代码风格

## 许可证

[许可证信息]

## 联系方式

- GitHub: [项目地址]
- Discord: [社区链接]
- 文档: [文档链接]