use crate::*;
use solana_sdk::system_program;

/// 初始化代币徽章的参数结构体
/// 代币徽章是一个特殊的标识系统，用于标记具有特定特性或权限的代币
/// 可用于实现白名单、黑名单或其他特殊的代币管理功能
#[derive(Debug, Parser)]
pub struct InitializeTokenBadgeParams {
    /// 代币铸造地址
    /// 要为其创建徽章的代币地址
    pub mint: Pubkey,
}

/// 执行初始化代币徽章操作
/// 
/// 此函数为指定的代币创建一个徽章，标记其特殊地位或特性。
/// 代币徽章系统允许协议对特定代币实行不同的策略，例如：
/// - 特殊的手续费结构
/// - 增强的安全检查
/// - 特殊的交易限制或优惠
/// - 合规性标识和追踪
/// 
/// # 参数
/// * `params` - 初始化参数，包含代币铸造地址
/// * `program` - Solana程序客户端，用于执行链上操作
/// * `transaction_config` - 交易配置，包含确认级别等设置
/// 
/// # 返回值
/// * `Result<()>` - 成功时返回空值，失败时返回错误
/// 
/// # 安全考虑
/// - 只有程序管理员可以创建代币徽章
/// - 代币忽章一旦创建就不能删除，需要谨慎操作
/// - 徽章会影响使用该代币的所有池对和交易
/// - 建议在正式环境中部署前充分测试
pub async fn execute_initialize_token_badge<C: Deref<Target = impl Signer> + Clone>(
    params: InitializeTokenBadgeParams,
    program: &Program<C>,
    transaction_config: RpcSendTransactionConfig,
) -> Result<()> {
    // 解构参数，获取代币铸造地址
    let InitializeTokenBadgeParams { mint } = params;

    // 生成代币徽章的PDA
    let (token_badge, _bump) = derive_token_badge_pda(mint);

    // 构建初始化代币徽章指令所需的账户列表
    let accounts = dlmm::client::accounts::InitializeTokenBadge {
        admin: program.payer(),                                     // 管理员账户（交易付款人）
        token_mint: mint,                                           // 代币铸造地址
        system_program: system_program::ID,                        // 系统程序
        token_badge,                                                // 新创建的代币徽章账户
    }
    .to_account_metas(None);

    // 构建初始化代币徽章指令的数据（无需额外参数）
    let data = dlmm::client::args::InitializeTokenBadge {}.data();

    // 构建完整的初始化代币徽章指令
    let instruction = Instruction {
        program_id: dlmm::ID,                                       // DLMM程序ID
        accounts,                                                   // 所需账户列表
        data,                                                       // 指令数据
    };

    // 构建并发送交易请求
    let request_builder = program.request();
    let signature = request_builder
        .instruction(instruction)                                   // 添加初始化徽章指令
        .send_with_spinner_and_config(transaction_config)          // 发送交易并等待确认
        .await;

    println!("Initialize token badge {}. Signature: {signature:#?}", mint);

    // 检查交易是否成功执行
    signature?;

    Ok(())
}
