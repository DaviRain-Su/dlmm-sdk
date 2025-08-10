use crate::*;

/// 设置预激活持续时间的参数结构体
/// 预激活持续时间决定了池对在激活前的准备阶段长度
/// 在这个阶段内，可能只允许特定的操作或参与者
#[derive(Debug, Parser)]
pub struct SetPreactivationDurationParam {
    /// 流动性池对的地址
    /// 需要设置预激活持续时间的池对
    pub lb_pair: Pubkey,
    /// 预激活持续时间（单位可能是秒或区块）
    /// 决定了池对在激活前的准备阶段长度
    pub pre_activation_duration: u16,
}

/// 执行设置预激活持续时间操作
/// 
/// 此函数允许池对创建者设置预激活阶段的持续时间。
/// 这是一个重要的时间控制机制，可用于：
/// - 等待充足的流动性积累
/// - 给予特定用户优先参与权
/// - 协调市场活动和发布时间
/// - 防止抢跑和不公平交易
pub async fn execute_set_pre_activation_duration<C: Deref<Target = impl Signer> + Clone>(
    params: SetPreactivationDurationParam,
    program: &Program<C>,
    transaction_config: RpcSendTransactionConfig,
) -> Result<()> {
    // 解构参数
    let SetPreactivationDurationParam {
        lb_pair,
        pre_activation_duration,
    } = params;

    // 构建设置预激活持续时间指令所需的账户列表
    let accounts = dlmm::client::accounts::SetPreActivationDuration {
        creator: program.payer(),                                   // 池对创建者（交易付款人）
        lb_pair,                                                    // 流动性池对账户
    }
    .to_account_metas(None);

    // 构建指令数据
    let data = dlmm::client::args::SetPreActivationDuration {
        pre_activation_duration: pre_activation_duration as u64,   // 转换为64位无符号整数
    }
    .data();

    // 构建完整的设置预激活持续时间指令
    let set_pre_activation_slot_duration_ix = Instruction {
        accounts,                                                   // 所需账户列表
        data,                                                       // 指令数据
        program_id: dlmm::ID,                                       // DLMM程序ID
    };

    // 构建并发送交易请求
    let request_builder = program.request();
    let signature = request_builder
        .instruction(set_pre_activation_slot_duration_ix)           // 添加设置指令
        .send_with_spinner_and_config(transaction_config)          // 发送交易并等待确认
        .await;

    println!("Set pre activation duration. Signature: {:#?}", signature);

    // 检查交易是否成功执行
    signature?;

    Ok(())
}
