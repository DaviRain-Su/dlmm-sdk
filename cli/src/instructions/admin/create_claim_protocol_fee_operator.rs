use crate::*;

/// 创建协议手续费领取操作员的参数结构体
/// 该操作将为指定地址创建协议手续费的领取权限
/// 只有授权的操作员才能从池对中提取协议手续费
#[derive(Debug, Parser)]
pub struct CreateClaimFeeOperatorParams {
    /// 新操作员的地址
    /// 该地址将获得领取协议手续费的权限
    #[clap(long)]
    pub operator: Pubkey,
}

/// 执行创建协议手续费领取操作员操作
/// 
/// 此函数允许管理员为指定地址创建协议手续费的领取权限。
/// 这是权限管理系统的核心功能，用于：
/// - 授权特定地址领取协议收入
/// - 建立分层的权限管理体系
/// - 实现多签名或团队管理
/// - 提高资金管理的安全性
/// 
/// # 参数
/// * `params` - 创建参数，包含新操作员的地址
/// * `program` - Solana程序客户端，用于执行链上操作
/// * `transaction_config` - 交易配置，包含确认级别等设置
/// 
/// # 返回值
/// * `Result<()>` - 成功时返回空值，失败时返回错误
/// 
/// # 安全考虑
/// - 只有程序管理员可以创建操作员
/// - 需要确保操作员地址的合法性和可信度
/// - 操作员一旦创建就拥有领取所有池对手续费的权限
/// - 建议使用多签名或硬件钱包作为操作员地址
pub async fn execute_create_claim_protocol_fee_operator<C: Deref<Target = impl Signer> + Clone>(
    params: CreateClaimFeeOperatorParams,
    program: &Program<C>,
    transaction_config: RpcSendTransactionConfig,
) -> Result<()> {
    // 解构参数，获取新操作员的地址
    let CreateClaimFeeOperatorParams { operator } = params;

    // 生成协议手续费领取操作员的PDA
    let (claim_fee_operator, _bump) = derive_claim_protocol_fee_operator_pda(operator);

    // 构建创建协议手续费领取操作员指令所需的账户列表
    let accounts = dlmm::client::accounts::CreateClaimProtocolFeeOperator {
        claim_fee_operator,                                         // 新创建的操作员账户
        operator,                                                   // 操作员地址
        admin: program.payer(),                                     // 管理员账户（交易付款人）
        system_program: anchor_lang::system_program::ID,            // 系统程序
    }
    .to_account_metas(None);

    // 构建创建操作员指令的数据（无需额外参数）
    let data = dlmm::client::args::CreateClaimProtocolFeeOperator {}.data();

    // 构建完整的创建协议手续费领取操作员指令
    let instruction = Instruction {
        program_id: dlmm::ID,                                       // DLMM程序ID
        accounts,                                                   // 所需账户列表
        data,                                                       // 指令数据
    };

    // 构建并发送交易请求
    let request_builder = program.request();
    let signature = request_builder
        .instruction(instruction)                                   // 添加创建操作员指令
        .send_with_spinner_and_config(transaction_config)          // 发送交易并等待确认
        .await;

    println!("Create claim protocol fee operator. Signature: {signature:#?}");

    // 检查交易是否成功执行
    signature?;

    Ok(())
}
