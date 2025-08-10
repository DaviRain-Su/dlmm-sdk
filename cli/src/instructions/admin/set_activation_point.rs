use crate::*;

/// 设置激活点的参数结构体
/// 激活点决定了流动性池对何时开始允许交易操作
/// 通常用于新创建的池对，设置一个未来的时间点或区块高度进行激活
#[derive(Debug, Parser)]
pub struct SetActivationPointParam {
    /// 流动性池对的地址
    /// 必须是已初始化的权限池对地址
    pub lb_pair: Pubkey,
    /// 激活点（时间戳或区块高度）
    /// 在这个点之前，池对将不允许交易，只能添加流动性
    pub activation_point: u64,
}

/// 执行设置激活点操作
/// 
/// 此函数允许管理员为流动性池对设置激活点。
/// 激活点是一个重要的时间控制机制，用于：
/// - 延迟激活新创建的池对
/// - 等待充足的流动性积累
/// - 协调市场营销和发布时间
/// - 防止抢跑等不公平交易行为
/// 
/// # 参数
/// * `params` - 包含池对地址和激活点的参数
/// * `program` - Solana程序客户端，用于执行链上操作
/// * `transaction_config` - 交易配置，包含确认级别等设置
/// 
/// # 返回值
/// * `Result<()>` - 成功时返回空值，失败时返回错误
/// 
/// # 安全考虑
/// - 只有程序管理员可以调用此函数
/// - 激活点一旦设置就无法撤销，只能推迟
/// - 设置太远的未来时间可能影响用户体验
/// - 需要确保激活点合理且符合项目计划
pub async fn execute_set_activation_point<C: Deref<Target = impl Signer> + Clone>(
    params: SetActivationPointParam,
    program: &Program<C>,
    transaction_config: RpcSendTransactionConfig,
) -> Result<()> {
    // 解构参数，获取池对地址和激活点
    let SetActivationPointParam {
        lb_pair,
        activation_point,
    } = params;

    // 构建设置激活点指令所需的账户列表
    // 只需要管理员账户和目标池对账户
    let accounts = dlmm::client::accounts::SetActivationPoint {
        admin: program.payer(),                                     // 管理员账户（必须是交易付款人）
        lb_pair,                                                    // 目标流动性池对账户
    }
    .to_account_metas(None);

    // 构建指令数据，包含激活点时间
    let data = dlmm::client::args::SetActivationPoint { 
        activation_point                                            // 激活点（时间戳或区块高度）
    }.data();

    // 构建完整的设置激活点指令
    let set_activation_point_ix = Instruction {
        accounts,                                                   // 所需账户列表
        data,                                                       // 指令数据
        program_id: dlmm::ID,                                       // DLMM程序ID
    };

    // 构建并发送交易请求
    let request_builder = program.request();
    let signature = request_builder
        .instruction(set_activation_point_ix)                       // 添加设置激活点指令
        .send_with_spinner_and_config(transaction_config)          // 发送交易并等待确认
        .await;

    println!("Set activation point. Signature: {:#?}", signature);

    // 检查交易是否成功执行
    signature?;

    Ok(())
}
