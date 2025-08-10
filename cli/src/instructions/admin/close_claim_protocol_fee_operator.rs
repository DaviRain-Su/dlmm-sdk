use crate::*;

/// 关闭协议手续费领取操作员的参数结构体
/// 该操作将撤销操作员的领取权限并释放账户的租金
/// 一旦关闭，该操作员将无法再领取协议手续费
#[derive(Debug, Parser)]
pub struct CloseClaimFeeOperatorParams {
    /// 要关闭的操作员地址
    /// 该操作员的领取权限将被撤销
    #[clap(long)]
    pub operator: Pubkey,
}

/// 执行关闭协议手续费领取操作员操作
/// 
/// 此函数允许管理员撤销指定操作员的协议手续费领取权限。
/// 这是一个重要的权限管理功能，用于：
/// - 撤销不再需要的操作员权限
/// - 回收账户租金以节约成本
/// - 管理操作员的生命周期
/// - 应对安全事件或人员变动
/// 
/// # 参数
/// * `params` - 关闭参数，包含要关闭的操作员地址
/// * `program` - Solana程序客户端，用于执行链上操作
/// * `transaction_config` - 交易配置，包含确认级别等设置
/// 
/// # 返回值
/// * `Result<()>` - 成功时返回空值，失败时返回错误
/// 
/// # 安全考虑
/// - 只有程序管理员可以执行此操作
/// - 操作员一旦关闭就无法恢复，需要重新创建
/// - 关闭后该操作员将无法领取任何协议手续费
/// - 租金将返还给管理员账户
pub async fn execute_close_claim_protocol_fee_operator<C: Deref<Target = impl Signer> + Clone>(
    params: CloseClaimFeeOperatorParams,
    program: &Program<C>,
    transaction_config: RpcSendTransactionConfig,
) -> Result<()> {
    // 解构参数，获取要关闭的操作员地址
    let CloseClaimFeeOperatorParams { operator } = params;

    // 生成协议手续费领取操作员的PDA
    let (claim_fee_operator, _bump) = derive_claim_protocol_fee_operator_pda(operator);

    // 构建关闭协议手续费领取操作员指令所需的账户列表
    let accounts = dlmm::client::accounts::CloseClaimProtocolFeeOperator {
        claim_fee_operator,                                         // 要关闭的操作员账户
        admin: program.payer(),                                     // 管理员账户（交易付款人）
        rent_receiver: program.payer(),                             // 租金接收者（通常是管理员）
    }
    .to_account_metas(None);

    // 构建关闭操作员指令的数据（无需额外参数）
    let data = dlmm::client::args::CloseClaimProtocolFeeOperator {}.data();

    // 构建完整的关闭协议手续费领取操作员指令
    let instruction = Instruction {
        program_id: dlmm::ID,                                       // DLMM程序ID
        accounts,                                                   // 所需账户列表
        data,                                                       // 指令数据
    };

    // 构建并发送交易请求
    let request_builder = program.request();
    let signature = request_builder
        .instruction(instruction)                                   // 添加关闭操作员指令
        .send_with_spinner_and_config(transaction_config)          // 发送交易并等待确认
        .await;

    println!("Close claim protocol fee operator. Signature: {signature:#?}");

    // 检查交易是否成功执行
    signature?;

    Ok(())
}
