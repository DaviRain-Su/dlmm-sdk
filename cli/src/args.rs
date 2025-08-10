use crate::instructions::{set_pair_status_permissionless::SetPairStatusPermissionlessParams, *};
use anchor_client::Cluster;
use clap::*;

/// 全局配置覆盖选项
#[derive(Parser, Debug)]
pub struct ConfigOverride {
    /// Cluster override
    /// 集群覆盖设置
    ///
    /// Values = mainnet, testnet, devnet, localnet.
    /// Default: mainnet
    #[clap(global = true, long = "provider.cluster", default_value_t = Cluster::Mainnet)]
    pub cluster: Cluster,
    /// Wallet override
    /// 钱包覆盖设置
    ///
    /// Example: /path/to/wallet/keypair.json
    /// Default: ~/.config/solana/id.json
    #[clap(
        global = true,
        long = "provider.wallet",
        default_value_t = String::from(shellexpand::tilde("~/.config/solana/id.json"))
    )]
    pub wallet: String,
    /// Priority fee
    /// 优先费用（用于加速交易）
    #[clap(global = true, long = "priority-fee", default_value_t = 0)]
    pub priority_fee: u64,
}

/// 解析流动性移除参数（bin_id, 移除百分比）
pub fn parse_bin_liquidity_removal(src: &str) -> Result<(i32, f64), Error> {
    let mut parsed_str: Vec<&str> = src.split(',').collect();

    let bps_to_remove = parsed_str
        .pop()
        .and_then(|s| s.parse::<f64>().ok())
        .ok_or_else(|| clap::error::Error::new(error::ErrorKind::InvalidValue))?;

    let bin_id = parsed_str
        .pop()
        .and_then(|s| s.parse::<i32>().ok())
        .ok_or_else(|| clap::error::Error::new(error::ErrorKind::InvalidValue))?;

    Ok((bin_id, bps_to_remove))
}

/// 解析流动性分配参数（delta_id, X代币分配比例, Y代币分配比例）
pub fn parse_bin_liquidity_distribution(src: &str) -> Result<(i32, f64, f64), Error> {
    let mut parsed_str: Vec<&str> = src.split(',').collect();

    let dist_y = parsed_str
        .pop()
        .and_then(|s| s.parse::<f64>().ok())
        .ok_or_else(|| clap::error::Error::new(error::ErrorKind::InvalidValue))?;

    let dist_x = parsed_str
        .pop()
        .and_then(|s| s.parse::<f64>().ok())
        .ok_or_else(|| clap::error::Error::new(error::ErrorKind::InvalidValue))?;

    let delta_id = parsed_str
        .pop()
        .and_then(|s| s.parse::<i32>().ok())
        .ok_or_else(|| clap::error::Error::new(error::ErrorKind::InvalidValue))?;

    Ok((delta_id, dist_x, dist_y))
}

/// 选择性舍入模式
#[derive(Debug, Clone, ValueEnum)]
pub enum SelectiveRounding {
    Up,    // 向上舍入
    Down,  // 向下舍入
    None,  // 不舍入
}

/// DLMM主要命令
#[derive(Parser, Debug)]
pub enum DLMMCommand {
    /// Create a new liquidity pair.
    /// 创建新的流动性交易对（版本2）
    InitializePair2(InitLbPair2Params),
    /// Create a new liquidity pair.
    /// 创建新的流动性交易对（版本1）
    InitializePair(InitLbPairParams),
    /// Initialize bin array for the given liquidity pair. Use InitializeBinArrayWithPriceRange or InitializeBinArrayWithBinRange for a more user friendly version.
    /// 初始化指定流动性对的bin数组。建议使用InitializeBinArrayWithPriceRange或InitializeBinArrayWithBinRange以获得更友好的体验
    InitializeBinArray(InitBinArrayParams),
    /// Initialize bin array for the given liquidity pair based on price range. For example: Initialize bin arrays for BTC/USDC from 20000 -> 30000 price.
    /// 基于价格范围初始化流动性对的bin数组。例如：为BTC/USDC初始化从20000到30000价格范围的bin数组
    InitializeBinArrayWithPriceRange(InitBinArrayWithPriceRangeParams),
    /// Initialize bin array for the given liquidity pair based on bin range. For example: Initialize bin arrays for BTC/USDC from bin 5660 -> 6600.
    /// 基于bin范围初始化流动性对的bin数组。例如：为BTC/USDC初始化从bin 5660到6600的bin数组
    InitializeBinArrayWithBinRange(InitBinArrayWithBinRangeParams),
    /// Initialize position for the given liquidity pair based on price range.
    /// 基于价格范围为指定流动性对初始化仓位
    InitializePositionWithPriceRange(InitPositionWithPriceRangeParams),
    /// Initialize position for the given liquidity pair based on bin range.
    /// 基于bin范围为指定流动性对初始化仓位
    InitializePosition(InitPositionParams),
    /// Deposit liquidity to the position of the given liquidity pair.
    /// 向指定流动性对的仓位存入流动性
    AddLiquidity(AddLiquidityParams),
    /// Remove liquidity from the position of the given liquidity pair.
    /// 从指定流动性对的仓位移除流动性
    RemoveLiquidity(RemoveLiquidityParams),
    /// Trade token X -> Y, or vice versa.
    /// 交易代币X到Y，或反向交易（精确输入数量）
    SwapExactIn(SwapExactInParams),
    /// 交易代币（精确输出数量）
    SwapExactOut(SwapExactOutParams),
    /// 带价格影响的交易
    SwapWithPriceImpact(SwapWithPriceImpactParams),
    /// Show information of the given liquidity pair.
    /// 显示指定流动性对的信息
    ShowPair(ShowPairParams),
    /// Show information of the given position.
    /// 显示指定仓位的信息
    ShowPosition(ShowPositionParams),
    /// 领取奖励
    ClaimReward(ClaimRewardParams),
    /// 更新奖励持续时间
    UpdateRewardDuration(UpdateRewardDurationParams),
    /// 更新奖励资助者
    UpdateRewardFunder(UpdateRewardFunderParams),
    /// Close liquidity position.
    /// 关闭流动性仓位
    ClosePosition(ClosePositionParams),
    /// Claim fee
    /// 领取手续费
    ClaimFee(ClaimFeeParams),
    /// Increase an oracle observation sample length
    /// 增加预言机观察样本长度
    IncreaseOracleLength(IncreaseOracleLengthParams),
    /// 显示预设参数
    ShowPresetParameter(ShowPresetAccountParams),
    /// 列出所有bin步长
    ListAllBinStep,
    /// 初始化可自定义的无需许可流动性对（版本1）
    InitializeCustomizablePermissionlessLbPair(InitCustomizablePermissionlessLbPairParam),
    /// 初始化可自定义的无需许可流动性对（版本2）
    InitializeCustomizablePermissionlessLbPair2(InitCustomizablePermissionlessLbPair2Param),
    /// Seed liquidity by operator
    /// 由操作员播种流动性
    SeedLiquidityByOperator(SeedLiquidityByOperatorParameters),
    /// 由操作员播种单个bin的流动性
    SeedLiquiditySingleBinByOperator(SeedLiquiditySingleBinByOperatorParameters),
    /// 无需许可设置交易对状态
    SetPairStatusPermissionless(SetPairStatusPermissionlessParams),
    /// 获取某个所有者的所有仓位
    GetAllPositionsForAnOwner(GetAllPositionsParams),
    /// 同步价格
    SyncPrice(SyncPriceParams),
    #[clap(flatten)]
    Admin(AdminCommand),
}

#[derive(Parser, Debug)]
#[clap(version, about, author)]
pub struct Cli {
    #[clap(flatten)]
    pub config_override: ConfigOverride,
    #[clap(subcommand)]
    pub command: DLMMCommand,
}

/// 管理员命令
#[derive(Debug, Parser)]
pub enum AdminCommand {
    /// Create a new permission liquidity pair. It allow liquidity fragmentation with exact bin step.
    /// 创建新的需要权限的流动性对。允许使用精确的bin步长进行流动性分片
    InitializePermissionPair(InitPermissionLbPairParameters),
    /// 设置交易对状态
    SetPairStatus(SetPairStatusParams),
    /// Remove liquidity by price range
    /// 按价格范围移除流动性
    RemoveLiquidityByPriceRange(RemoveLiquidityByPriceRangeParameters),
    /// 设置激活点
    SetActivationPoint(SetActivationPointParam),
    /// 提取协议费用
    WithdrawProtocolFee(WithdrawProtocolFeeParams),
    /// 初始化奖励
    InitializeReward(InitializeRewardParams),
    /// 资助奖励池
    FundReward(FundRewardParams),
    /// 初始化预设参数
    InitializePresetParameter(InitPresetParameters),
    /// 关闭预设参数账户
    ClosePresetParameter(ClosePresetAccountParams),
    /// 设置预激活持续时间
    SetPreActivationDuration(SetPreactivationDurationParam),
    /// 设置预激活交换地址
    SetPreActivationSwapAddress(SetPreactivationSwapAddressParam),
    /// 初始化代币徽章
    InitializeTokenBadge(InitializeTokenBadgeParams),
    /// 创建协议费用领取操作员
    CreateClaimProtocolFeeOperator(CreateClaimFeeOperatorParams),
    /// 关闭协议费用领取操作员
    CloseClaimProtocolFeeOperator(CloseClaimFeeOperatorParams),
    /// 更新基础费率
    UpdateBaseFee(UpdateBaseFeeParams),
}
