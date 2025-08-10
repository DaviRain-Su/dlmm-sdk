use crate::*;

/// 初始化奖励系统的参数结构体
/// 奖励系统允许为流动性池对设置额外的代币奖励，激励流动性提供者参与
/// 每个池对最多可以支持多个不同的奖励代币
#[derive(Debug, Parser)]
pub struct InitializeRewardParams {
    /// 流动性池对的地址
    /// 该池对将被设置奖励系统
    pub lb_pair: Pubkey,
    /// 奖励代币的铸造地址
    /// 用于发放给流动性提供者的奖励代币
    pub reward_mint: Pubkey,
    /// 奖励索引
    /// 用于区分同一池对中的不同奖励代币，通常从0开始
    pub reward_index: u64,
    /// 奖励持续时间（以秒为单位）
    /// 决定了奖励的分发期限，超过该时间后将停止发放
    pub reward_duration: u64,
    /// 奖励资助者的地址
    /// 只有该地址可以为奖励系统追加资金
    pub funder: Pubkey,
}

/// 执行初始化奖励系统操作
/// 
/// 此函数为指定的流动性池对初始化一个奖励系统。
/// 奖励系统是DEX激励机制的重要组成部分，可以：
/// - 吸引更多流动性提供者参与
/// - 提高池对的流动性和交易量
/// - 激励特定代币对的交易活跃度
/// - 支持新项目的冷启动和推广
/// 
/// # 参数
/// * `params` - 奖励系统初始化参数
/// * `program` - Solana程序客户端，用于执行链上操作
/// * `transaction_config` - 交易配置，包含确认级别等设置
/// 
/// # 返回值
/// * `Result<()>` - 成功时返回空值，失败时返回错误
/// 
/// # 安全考虑
/// - 只有程序管理员可以初始化奖励系统
/// - 奖励索引必须唯一且从0开始递增
/// - 奖励代币必须是有效的SPL Token或Token-2022
/// - 资助者地址必须是有效的Solana账户地址
pub async fn execute_initialize_reward<C: Deref<Target = impl Signer> + Clone>(
    params: InitializeRewardParams,
    program: &Program<C>,
    transaction_config: RpcSendTransactionConfig,
) -> Result<()> {
    // 解构初始化奖励参数
    let InitializeRewardParams {
        lb_pair,
        reward_mint,
        reward_index,
        reward_duration,
        funder,
    } = params;

    // 生成奖励金库的PDA，用于存放奖励代币
    // 每个奖励索引都有对应的独立金库
    let (reward_vault, _bump) = derive_reward_vault_pda(lb_pair, reward_index);
    
    // 生成事件权限账户PDA，用于记录奖励初始化事件
    let (event_authority, _bump) = derive_event_authority_pda();

    let rpc_client = program.rpc();
    // 获取奖励代币的账户信息，用于确定代币程序ID
    let reward_mint_account = rpc_client.get_account(&reward_mint).await?;

    // 生成奖励代币的徽章PDA
    let (token_badge, _bump) = derive_token_badge_pda(reward_mint);
    // 检查代币徽章是否存在，如果不存在则使用程序ID作为默认值
    // 这允许同时支持带有特殊标识的代币和普通代币
    let token_badge = rpc_client
        .get_account(&token_badge)
        .await
        .ok()
        .map(|_| token_badge)
        .or(Some(dlmm::ID));

    // 构建初始化奖励系统所需的账户列表
    let accounts = dlmm::client::accounts::InitializeReward {
        lb_pair,                                                    // 流动性池对账户
        reward_vault,                                               // 奖励金库账户
        reward_mint,                                                // 奖励代币铸造地址
        admin: program.payer(),                                     // 管理员账户（交易付款人）
        token_program: reward_mint_account.owner,                   // 奖励代币的程序ID
        token_badge,                                                // 奖励代币徽章（如果存在）
        rent: solana_sdk::sysvar::rent::ID,                        // 租金系统变量
        system_program: solana_sdk::system_program::ID,            // 系统程序
        event_authority,                                            // 事件权限账户
        program: dlmm::ID,                                          // DLMM程序ID
    }
    .to_account_metas(None);

    // 构建初始化奖励指令的数据
    let data = dlmm::client::args::InitializeReward {
        reward_index,                                               // 奖励索引
        reward_duration,                                            // 奖励持续时间
        funder,                                                     // 奖励资助者地址
    }
    .data();

    // 构建完整的初始化奖励系统指令
    let instruction = Instruction {
        program_id: dlmm::ID,                                       // DLMM程序ID
        accounts,                                                   // 所需账户列表
        data,                                                       // 指令数据
    };

    // 构建并发送交易请求
    let request_builder = program.request();
    let signature = request_builder
        .instruction(instruction)                                   // 添加初始化奖励指令
        .send_with_spinner_and_config(transaction_config)          // 发送交易并等待确认
        .await;

    println!("Initialize reward. Signature: {signature:#?}");

    // 检查交易是否成功执行
    signature?;

    Ok(())
}
