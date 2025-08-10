use crate::*;

/// 设置预激活交易地址的参数结构体
/// 预激活交易地址是在池对正式激活前唯一可以进行交易的地址
/// 这可能用于特殊的发布机制或授权交易策略
#[derive(Debug, Parser)]
pub struct SetPreactivationSwapAddressParam {
    /// 流动性池对的地址
    pub lb_pair: Pubkey,
    /// 预激活交易地址
    pub pre_activation_swap_address: Pubkey,
}

/// 执行设置预激活交易地址操作
/// 
/// 此函数允许池对创建者设置一个特殊的地址，该地址在池对正式激活前可以进行交易。
pub async fn execute_set_pre_activation_swap_address<C: Deref<Target = impl Signer> + Clone>(
    params: SetPreactivationSwapAddressParam,
    program: &Program<C>,
    transaction_config: RpcSendTransactionConfig,
) -> Result<()> {
    // 解构参数
    let SetPreactivationSwapAddressParam {
        lb_pair,
        pre_activation_swap_address,
    } = params;

    // 构建设置预激活交易地址指令所需的账户列表
    let accounts = dlmm::client::accounts::SetPreActivationSwapAddress {
        creator: program.payer(),                                   // 池对创建者
        lb_pair,                                                    // 流动性池对账户
    }
    .to_account_metas(None);

    // 构建指令数据
    let data = dlmm::client::args::SetPreActivationSwapAddress {
        pre_activation_swap_address,                                // 预激活交易地址
    }
    .data();

    // 构建完整的设置预激活交易地址指令
    let set_pre_activation_swap_address_ix = Instruction {
        accounts,                                                   // 所需账户列表
        data,                                                       // 指令数据
        program_id: dlmm::ID,                                       // DLMM程序ID
    };

    // 构建并发送交易请求
    let request_builder = program.request();
    let signature = request_builder
        .instruction(set_pre_activation_swap_address_ix)            // 添加设置指令
        .send_with_spinner_and_config(transaction_config)          // 发送交易并等待确认
        .await;

    println!(
        "Set pre activation swap address. Signature: {:#?}",
        signature
    );

    // 检查交易是否成功执行
    signature?;

    Ok(())
}
