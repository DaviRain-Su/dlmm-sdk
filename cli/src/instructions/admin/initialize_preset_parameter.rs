use anchor_lang::Discriminator;
use commons::dlmm::{accounts::PresetParameter2, types::InitPresetParameters2Ix};
use solana_client::{
    rpc_config::{RpcAccountInfoConfig, RpcProgramAccountsConfig},
    rpc_filter::{Memcmp, RpcFilterType},
};

use crate::*;

/// 初始化预设参数的结构体
/// 预设参数定义了流动性池对的费用结构和交易特性
/// 这些参数决定了池对的经济模型和交易费用的动态调整机制
#[derive(Debug, Parser)]
pub struct InitPresetParameters {
    /// 箱子步长，表示价格的增减幅度
    /// 决定了相邻价格箱子之间的最小价格变动
    pub bin_step: u16,
    /// 用于基础手续费计算的因子，公式：base_fee_rate = base_factor * bin_step
    /// 该参数控制了基础手续费的水平
    pub base_factor: u16,
    /// 过滤周期，决定高频交易的时间窗口
    /// 用于检测和应对短时间内的频繁交易
    pub filter_period: u16,
    /// 衰减周期，决定波动性手续费何时开始衰减/降低
    /// 在市场波动性降低后，手续费会逐渐回归正常水平
    pub decay_period: u16,
    /// 减少因子，控制波动性手续费的下降速度
    /// 该值越大，手续费下降越快
    pub reduction_factor: u16,
    /// 用于根据市场动态缩放可变手续费组成部分
    /// 该参数影响手续费对市场波动性的敏感度
    pub variable_fee_control: u32,
    /// 可积累的最大穿越箱子数量，用于限制波动性手续费的上限
    /// 防止手续费在极端情况下过高
    pub max_volatility_accumulator: u32,
    /// 协议保留的交易手续费比例，公式：protocol_swap_fee = protocol_share * total_swap_fee
    /// 该参数决定了协议和流动性提供者之间的手续费分配
    pub protocol_share: u16,
    /// 基础手续费幂因子
    /// 用于调整手续费的非线性特性
    pub base_fee_power_factor: u8,
}

/// 执行初始化预设参数操作
/// 
/// 此函数创建一个新的预设参数模板，定义了流动性池对的费用结构和交易特性。
/// 预设参数是整个DLMM系统的核心配置，用于：
/// - 定义不同类型池对的经济模型
/// - 控制手续费的动态调整机制
/// - 平衡流动性提供者和交易者的利益
/// - 适应不同的市场条件和资产特性
/// 
/// # 参数
/// * `params` - 预设参数配置，包含所有费用和特性设置
/// * `program` - Solana程序客户端，用于执行链上操作
/// * `transaction_config` - 交易配置，包含确认级别等设置
/// 
/// # 返回值
/// * `Result<Pubkey>` - 成功时返回创建的预设参数地址，失败时返回错误
/// 
/// # 安全考虑
/// - 只有程序管理员可以创建预设参数
/// - 参数配置会影响所有使用该模板的池对
/// - 建议在生产环境中充分测试参数组合
/// - 需要谨慎考虑对整个生态系统的影响
pub async fn execute_initialize_preset_parameter<C: Deref<Target = impl Signer> + Clone>(
    params: InitPresetParameters,
    program: &Program<C>,
    transaction_config: RpcSendTransactionConfig,
) -> Result<Pubkey> {
    // 解构预设参数配置，获取所有必要的费用和特性设置
    let InitPresetParameters {
        base_factor,
        bin_step,
        decay_period,
        filter_period,
        max_volatility_accumulator,
        protocol_share,
        reduction_factor,
        variable_fee_control,
        base_fee_power_factor,
    } = params;

    let rpc_client = program.rpc();

    // 查询现有的预设参数v2数量，用于确定新参数的索引
    // 这确保了每个预设参数都有唯一的索引标识符
    let preset_parameter_v2_count = rpc_client
        .get_program_accounts_with_config(
            &dlmm::ID,
            RpcProgramAccountsConfig {
                filters: Some(vec![RpcFilterType::Memcmp(Memcmp::new_base58_encoded(
                    0,
                    &PresetParameter2::DISCRIMINATOR,              // 过滤出预设参数v2账户
                ))]),
                account_config: RpcAccountInfoConfig {
                    encoding: Some(UiAccountEncoding::Base64),
                    data_slice: Some(UiDataSliceConfig {
                        offset: 0,
                        length: 0,                                  // 只需要账户数量，不需要数据
                    }),
                    ..Default::default()
                },
                ..Default::default()
            },
        )
        .await?
        .len();

    // 使用当前数量作为新预设参数的索引
    let index = preset_parameter_v2_count as u16;

    // 生成新预设参数的PDA
    let (preset_parameter, _bump) =
        derive_preset_parameter_pda_v2(preset_parameter_v2_count as u16);

    // 构建初始化预设参数指令所需的账户列表
    let accounts = dlmm::client::accounts::InitializePresetParameter2 {
        preset_parameter,                                           // 新创建的预设参数账户
        admin: program.payer(),                                     // 管理员账户（交易付款人）
        system_program: solana_sdk::system_program::ID,            // 系统程序
    }
    .to_account_metas(None);

    // 构建初始化预设参数指令的数据
    let data = dlmm::client::args::InitializePresetParameter2 {
        ix: InitPresetParameters2Ix {
            index,                                                  // 预设参数索引
            bin_step,                                               // 箱子步长
            base_factor,                                            // 基础因子
            filter_period,                                          // 过滤周期
            decay_period,                                           // 衰减周期
            reduction_factor,                                       // 减少因子
            variable_fee_control,                                   // 可变手续费控制
            max_volatility_accumulator,                             // 最大波动性积累器
            protocol_share,                                         // 协议分成
            base_fee_power_factor,                                  // 基础手续费幂因子
        },
    }
    .data();

    // 构建完整的初始化预设参数指令
    let init_preset_param_ix = Instruction {
        program_id: dlmm::ID,                                       // DLMM程序ID
        accounts,                                                   // 所需账户列表
        data,                                                       // 指令数据
    };

    // 构建并发送交易请求
    let request_builder = program.request();
    let signature = request_builder
        .instruction(init_preset_param_ix)                          // 添加初始化指令
        .send_with_spinner_and_config(transaction_config)          // 发送交易并等待确认
        .await;

    println!(
        "Initialize preset parameter {}. Signature: {signature:#?}",
        preset_parameter
    );

    // 检查交易是否成功执行
    signature?;

    // 返回成功创建的预设参数地址
    Ok(preset_parameter)
}
