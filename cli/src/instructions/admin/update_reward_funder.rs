use crate::*;

/// 更新奖励资助者的参数结构体
/// 该功能允许管理员更改奖励系统的授权资助者
/// 只有授权的资助者才能为奖励系统添加资金
#[derive(Debug, Parser)]
pub struct UpdateRewardFunderParams {
    /// 流动性池对的地址
    /// 需要更新资助者的池对
    pub lb_pair: Pubkey,
    /// 奖励索引
    /// 指定要更新资助者的奖励系统索引
    pub reward_index: u64,
    /// 新的资助者地址
    /// 新授权的资助者，只有该地址可以为奖励系统添加资金
    pub funder: Pubkey,
}

/// 执行更新奖励资助者操作
/// 
/// 此函数允许管理员更改指定奖励系统的授权资助者。
/// 这是一个重要的权限管理功能，用于：
/// - 转移奖励系统的控制权
/// - 更换资助者角色和责任
/// - 建立多元化的资助机制
/// - 应对组织架构变化
/// 
/// # 参数
/// * `params` - 更新参数，包括池对、奖励索引和新资助者地址
/// * `program` - Solana程序客户端，用于执行链上操作
/// * `transaction_config` - 交易配置，包含确认级别等设置
/// 
/// # 返回值
/// * `Result<()>` - 成功时返回空值，失败时返回错误
/// 
/// # 安全考虑
/// - 只有程序管理员可以执行此操作
/// - 新资助者地址必须是有效的Solana账户
/// - 更改后只有新资助者可以添加资金
/// - 建议在更改前通知相关利益方
/// - 需要谨慎管理资助者权限以防止滥用
pub async fn execute_update_reward_funder<C: Deref<Target = impl Signer> + Clone>(
    params: UpdateRewardFunderParams,
    program: &Program<C>,
    transaction_config: RpcSendTransactionConfig,
) -> Result<()> {
    // 解构更新奖励资助者参数
    let UpdateRewardFunderParams {
        lb_pair,
        reward_index,
        funder,
    } = params;

    // 生成事件权限账户PDA，用于记录资助者更新事件
    let (event_authority, _bump) = derive_event_authority_pda();

    // 构建更新奖励资助者指令所需的账户列表
    let accounts = dlmm::client::accounts::UpdateRewardFunder {
        lb_pair,                                                    // 流动性池对账户
        admin: program.payer(),                                     // 管理员账户（交易付款人）
        event_authority,                                            // 事件权限账户
        program: dlmm::ID,                                          // DLMM程序ID
    }
    .to_account_metas(None);

    // 构建更新奖励资助者指令的数据
    let data = dlmm::client::args::UpdateRewardFunder {
        reward_index,                                               // 奖励索引
        new_funder: funder,                                         // 新的资助者地址
    }
    .data();

    // 构建完整的更新奖励资助者指令
    let ix = Instruction {
        program_id: dlmm::ID,                                       // DLMM程序ID
        accounts,                                                   // 所需账户列表
        data,                                                       // 指令数据
    };

    // 构建并发送交易请求
    let request_builder = program.request();
    let signature = request_builder
        .instruction(ix)                                            // 添加更新资助者指令
        .send_with_spinner_and_config(transaction_config)          // 发送交易并等待确认
        .await;

    println!("Update reward funder. Signature: {:#?}", signature);

    // 检查交易是否成功执行
    signature?;

    Ok(())
}
