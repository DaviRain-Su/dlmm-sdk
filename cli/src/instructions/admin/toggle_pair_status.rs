use crate::*;

/// 设置流动性池对状态的参数结构体
/// 此功能允许管理员启用或禁用特定的流动性池对
/// 禁用的池对将无法进行交易操作，但现有的流动性提供者仍可提取资金
#[derive(Debug, Parser)]
pub struct SetPairStatusParams {
    /// 流动性池对的地址
    /// 必须是有效的且已初始化的池对地址
    pub lb_pair: Pubkey,
    /// 池对状态：0 表示启用，1 表示禁用
    /// 启用状态允许所有正常交易操作，禁用状态会阻止新的交易
    pub pair_status: u8,
}

/// 执行设置流动性池对状态操作
/// 
/// 此函数允许管理员控制流动性池对的启用/禁用状态。
/// 这是一个重要的风险管理工具，可用于：
/// - 紧急情况下暂停交易
/// - 维护期间禁用池对
/// - 逐步迁移到新版本
/// 
/// # 参数
/// * `params` - 包含池对地址和目标状态的参数
/// * `program` - Solana程序客户端，用于执行链上操作
/// * `transaction_config` - 交易配置，包含确认级别等设置
/// 
/// # 返回值
/// * `Result<()>` - 成功时返回空值，失败时返回错误
/// 
/// # 安全考虑
/// - 只有程序管理员可以调用此函数
/// - 状态更改会立即生效
/// - 禁用池对不会影响现有流动性的提取
/// - 建议在状态更改前通知用户和流动性提供者
pub async fn execute_set_pair_status<C: Deref<Target = impl Signer> + Clone>(
    params: SetPairStatusParams,
    program: &Program<C>,
    transaction_config: RpcSendTransactionConfig,
) -> Result<()> {
    // 解构参数，获取池对地址和目标状态
    let SetPairStatusParams {
        lb_pair,
        pair_status,
    } = params;

    // 构建设置池对状态指令所需的账户列表
    // 只需要管理员账户和目标池对账户
    let accounts = dlmm::client::accounts::SetPairStatus {
        admin: program.payer(),                                     // 管理员账户（必须是交易付款人）
        lb_pair,                                                    // 目标流动性池对账户
    }
    .to_account_metas(None);

    // 构建指令数据，包含新的状态值
    let data = dlmm::client::args::SetPairStatus {
        status: pair_status,                                        // 新的池对状态（0启用，1禁用）
    }
    .data();

    // 构建完整的设置池对状态指令
    let instruction = Instruction {
        program_id: dlmm::ID,                                       // DLMM程序ID
        accounts,                                                   // 所需账户列表
        data,                                                       // 指令数据
    };

    // 构建并发送交易请求
    let request_builder = program.request();
    let signature = request_builder
        .instruction(instruction)                                   // 添加设置状态指令
        .send_with_spinner_and_config(transaction_config)          // 发送交易并等待确认
        .await;

    println!("Set pair status. Signature: {:#?}", signature);

    // 检查交易是否成功执行
    signature?;

    Ok(())
}
