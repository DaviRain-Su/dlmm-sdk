use crate::*;
use commons::dlmm::accounts::{BinArray, LbPair};

/// 更新奖励持续时间的参数结构体
/// 该功能允许管理员调整奖励系统的分发周期
/// 可用于延长或缩短奖励的有效期，灵活调整激励策略
#[derive(Debug, Parser)]
pub struct UpdateRewardDurationParams {
    /// 流动性池对的地址
    /// 需要更新奖励持续时间的池对
    pub lb_pair: Pubkey,
    /// 奖励索引
    /// 指定要更新的奖励系统索引
    pub reward_index: u64,
    /// 新的奖励持续时间（以秒为单位）
    /// 决定了奖励将在多长时间内分发给流动性提供者
    pub reward_duration: u64,
}

/// 执行更新奖励持续时间操作
/// 
/// 此函数允许管理员修改指定奖励系统的分发周期。
/// 这是一个重要的管理工具，可用于：
/// - 根据市场情况调整激励周期
/// - 延长奖励分发以提高吸引力
/// - 缩短奖励周期以增加分发速度
/// - 适应不同阶段的运营需要
/// 
/// # 参数
/// * `params` - 更新参数，包括池对、奖励索引和新持续时间
/// * `program` - Solana程序客户端，用于执行链上操作
/// * `transaction_config` - 交易配置，包含确认级别等设置
/// 
/// # 返回值
/// * `Result<()>` - 成功时返回空值，失败时返回错误
/// 
/// # 安全考虑
/// - 只有程序管理员可以执行此操作
/// - 新持续时间必须大于0且合理
/// - 更新会影响奖励的分发速度和总量
/// - 建议在更新前评估对流动性提供者的影响
pub async fn execute_update_reward_duration<C: Deref<Target = impl Signer> + Clone>(
    params: UpdateRewardDurationParams,
    program: &Program<C>,
    transaction_config: RpcSendTransactionConfig,
) -> Result<()> {
    // 解构更新奖励持续时间参数
    let UpdateRewardDurationParams {
        lb_pair,
        reward_index,
        reward_duration,
    } = params;

    let rpc_client = program.rpc();
    // 获取并反序列化流动性池对状态数据
    // 需要这些数据来获取活跃箱子ID和其他相关信息
    let lb_pair_state: LbPair = rpc_client
        .get_account_and_deserialize(&lb_pair, |account| {
            Ok(bytemuck::pod_read_unaligned(&account.data[8..]))     // 跳过8字节的账户判别符
        })
        .await?;

    // 计算当前活跃箱子数组的索引和地址
    // 更新奖励持续时间需要访问活跃箱子数组来更新奖励信息
    let active_bin_array_idx = BinArray::bin_id_to_bin_array_index(lb_pair_state.active_id)?;
    let (bin_array, _bump) = derive_bin_array_pda(lb_pair, active_bin_array_idx as i64);

    // 生成事件权限账户PDA，用于记录奖励更新事件
    let (event_authority, _bump) = derive_event_authority_pda();

    // 构建更新奖励持续时间指令所需的账户列表
    let accounts = dlmm::client::accounts::UpdateRewardDuration {
        lb_pair,                                                    // 流动性池对账户
        admin: program.payer(),                                     // 管理员账户（交易付款人）
        bin_array,                                                  // 活跃箱子数组账户
        event_authority,                                            // 事件权限账户
        program: dlmm::ID,                                          // DLMM程序ID
    }
    .to_account_metas(None);

    // 构建更新奖励持续时间指令的数据
    let data = dlmm::client::args::UpdateRewardDuration {
        reward_index,                                               // 奖励索引
        new_duration: reward_duration,                              // 新的奖励持续时间
    }
    .data();

    // 构建完整的更新奖励持续时间指令
    let ix = Instruction {
        program_id: dlmm::ID,                                       // DLMM程序ID
        accounts,                                                   // 所需账户列表
        data,                                                       // 指令数据
    };

    // 构建并发送交易请求
    let request_builder = program.request();
    let signature = request_builder
        .instruction(ix)                                            // 添加更新持续时间指令
        .send_with_spinner_and_config(transaction_config)          // 发送交易并等待确认
        .await;

    println!("Update reward duration. Signature: {:#?}", signature);

    // 检查交易是否成功执行
    signature?;

    Ok(())
}
