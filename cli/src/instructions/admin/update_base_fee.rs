use crate::*;

/// 更新基础手续费的参数结构体
/// 该功能允许管理员调整池对的基础手续费率
/// 基础手续费是所有交易的基础成本，不受市场波动性影响
#[derive(Debug, Parser)]
pub struct UpdateBaseFeeParams {
    /// 流动性池对的地址
    /// 需要更新基础手续费的池对
    pub lb_pair: Pubkey,
    /// 新的基础手续费率（以基点为单位）
    /// 1基点 = 0.01%，用于设置新的基础手续费水平
    pub base_fee_bps: u16,
}

/// 执行更新基础手续费操作
/// 
/// 此函数允许管理员调整指定池对的基础手续费率。
/// 这是一个关键的经济参数调整功能，影响所有交易者。
pub async fn execute_update_base_fee<C: Deref<Target = impl Signer> + Clone>(
    params: UpdateBaseFeeParams,
    program: &Program<C>,
    transaction_config: RpcSendTransactionConfig,
) -> Result<()> {
    // 解构参数
    let UpdateBaseFeeParams {
        lb_pair,
        base_fee_bps,
    } = params;

    let rpc_client = program.rpc();

    // 获取池对账户数据
    let pair_account = rpc_client.get_account(&lb_pair).await?;

    // 反序列化池对状态数据，获取当前的参数设置
    let lb_pair_state = LbPair::try_deserialize(&mut pair_account.data.as_ref())?;

    // 根据新的基础手续费率计算相应的基础因子和幂因子
    let (base_factor, base_fee_power_factor) =
        compute_base_factor_from_fee_bps(lb_pair_state.bin_step, base_fee_bps)?;

    // 构建更新基础手续费参数指令的数据
    let ix_data = dlmm::client::args::UpdateBaseFeeParameters {
        fee_parameter: BaseFeeParameter {
            protocol_share: lb_pair_state.parameters.protocol_share, // 保持原有协议分成
            base_factor,                                            // 新的基础因子
            base_fee_power_factor,                                  // 新的基础手续费幂因子
        },
    }
    .data();

    // 生成事件权限账户PDA
    let event_authority = derive_event_authority_pda().0;

    // 构建更新基础手续费参数指令所需的账户列表
    let accounts = dlmm::client::accounts::UpdateBaseFeeParameters {
        lb_pair,                                                    // 流动性池对账户
        admin: program.payer(),                                     // 管理员账户
        event_authority,                                            // 事件权限账户
        program: dlmm::ID,                                          // DLMM程序ID
    }
    .to_account_metas(None);

    // 构建完整的更新基础手续费参数指令
    let ix = Instruction {
        program_id: program.id(),                                   // 程序ID
        data: ix_data,                                              // 指令数据
        accounts: accounts.to_vec(),                                // 账户列表
    };

    // 构建并发送交易请求
    let request_builder = program.request();
    let signature = request_builder
        .instruction(ix)                                            // 添加更新指令
        .send_with_spinner_and_config(transaction_config)          // 发送交易并等待确认
        .await;

    println!("Update base fee. Signature: {:#?}", signature);

    // 检查交易是否成功执行
    signature?;

    Ok(())
}
