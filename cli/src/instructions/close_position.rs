use crate::*;

/// 关闭仓位的参数结构体
/// Parameters for closing position
#[derive(Debug, Parser)]
pub struct ClosePositionParams {
    /// 仓位地址
    /// Position address
    pub position: Pubkey,
}

/// 执行关闭仓位指令
/// Executes the close position instruction
/// 
/// # 参数 / Parameters
/// * `params` - 关闭仓位的参数 / Parameters for closing position
/// * `program` - Solana程序引用 / Solana program reference
/// * `transaction_config` - 交易配置 / Transaction configuration
/// 
/// # 功能说明 / Functionality
/// 关闭一个空的流动性仓位，回收租金到指定账户
/// Closes an empty liquidity position and recovers rent to the specified account
pub async fn execute_close_position<C: Deref<Target = impl Signer> + Clone>(
    params: ClosePositionParams,
    program: &Program<C>,
    transaction_config: RpcSendTransactionConfig,
) -> Result<()> {
    let ClosePositionParams { position } = params;

    let rpc_client = program.rpc();
    
    // 获取仓位状态数据
    // Get position state data
    let position_state: PositionV2 = rpc_client
        .get_account_and_deserialize(&position, |account| {
            Ok(bytemuck::pod_read_unaligned(&account.data[8..]))
        })
        .await?;

    // 获取仓位覆盖的所有bin数组账户元数据
    // Get all bin array account metadata covered by the position
    let bin_arrays_account_meta = position_state.get_bin_array_accounts_meta_coverage()?;

    // 派生事件权限PDA
    // Derive event authority PDA
    let (event_authority, _bump) = derive_event_authority_pda();

    // 构建主要账户信息
    // Build main account information
    let main_accounts = dlmm::client::accounts::ClosePosition2 {
        sender: position_state.owner,        // 发送者（仓位所有者）/ Sender (position owner)
        rent_receiver: position_state.owner, // 租金接收者（仓位所有者）/ Rent receiver (position owner)
        position,                           // 要关闭的仓位账户 / Position account to close
        event_authority,                    // 事件权限 / Event authority
        program: dlmm::ID,                  // DLMM程序ID / DLMM program ID
    }
    .to_account_metas(None);

    // 构建关闭仓位指令数据（无需额外参数）
    // Build close position instruction data (no additional parameters needed)
    let data = dlmm::client::args::ClosePosition2 {}.data();
    
    // 设置计算预算限制
    // Set compute budget limit
    let compute_budget_ix = ComputeBudgetInstruction::set_compute_unit_limit(1_400_000);

    // 组合所有必需的账户
    // Combine all required accounts
    let accounts = [main_accounts.to_vec(), bin_arrays_account_meta].concat();

    // 创建关闭仓位指令
    // Create close position instruction
    let close_position_ix = Instruction {
        program_id: dlmm::ID,
        accounts,
        data,
    };

    // 构建并发送交易
    // Build and send transaction
    let request_builder = program.request();
    let signature = request_builder
        .instruction(compute_budget_ix)     // 添加计算预算指令 / Add compute budget instruction
        .instruction(close_position_ix)     // 添加关闭仓位指令 / Add close position instruction
        .send_with_spinner_and_config(transaction_config)
        .await;

    println!("Close position. Signature: {:#?}", signature);

    signature?;

    Ok(())
}
