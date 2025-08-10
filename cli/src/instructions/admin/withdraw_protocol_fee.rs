use anchor_spl::associated_token::get_associated_token_address_with_program_id;
use commons::dlmm::accounts::LbPair;

use crate::*;

/// 提取协议手续费的参数结构体
/// 协议手续费是从每笔交易中收取的一部分手续费，用于维持协议运行
/// 只有授权的操作员或管理员才能提取这些费用
#[derive(Debug, Parser)]
pub struct WithdrawProtocolFeeParams {
    /// 流动性池对的地址
    /// 从该池对中提取积累的协议手续费
    pub lb_pair: Pubkey,
}

/// 执行提取协议手续费操作
/// 
/// 此函数允许授权操作员从指定的流动性池对中提取积累的协议手续费。
/// 协议手续费是协议收入的主要来源，用于：
/// - 支付开发和维护成本
/// - 激励生态系统参与者
/// - 财务储备和风险管理
/// - 支持新功能开发
/// 
/// # 参数
/// * `params` - 包含池对地址的参数
/// * `program` - Solana程序客户端，用于执行链上操作
/// * `transaction_config` - 交易配置，包含确认级别等设置
/// 
/// # 返回值
/// * `Result<()>` - 成功时返回空值，失败时返回错误
/// 
/// # 安全考虑
/// - 只有授权的操作员可以执行此操作
/// - 该操作会检查操作员权限和身份验证
/// - 提取的费用将转入操作员的关联代币账户
/// - 支持Token-2022标准和转账钩子功能
pub async fn execute_withdraw_protocol_fee<C: Deref<Target = impl Signer> + Clone>(
    params: WithdrawProtocolFeeParams,
    program: &Program<C>,
    transaction_config: RpcSendTransactionConfig,
) -> Result<()> {
    // 解构参数，获取池对地址
    let WithdrawProtocolFeeParams { lb_pair } = params;

    let rpc_client = program.rpc();

    // 获取并反序列化流动性池对状态数据
    // 需要这些数据来获取代币铸造地址、储备金库地址等信息
    let lb_pair_state: LbPair = rpc_client
        .get_account_and_deserialize(&lb_pair, |account| {
            Ok(bytemuck::pod_read_unaligned(&account.data[8..]))     // 跳过8字节的账户判别符
        })
        .await?;

    // 获取代币X和代币Y的程序ID，支持SPL Token和Token-2022标准
    let [token_x_program, token_y_program] = lb_pair_state.get_token_programs()?;

    // 生成操作员接收代币X的关联代币账户地址
    // 协议手续费中的代币X部分将转入这个账户
    let receiver_token_x = get_associated_token_address_with_program_id(
        &program.payer(),                                           // 操作员地址
        &lb_pair_state.token_x_mint,                                // 代币X铸造地址
        &token_x_program,                                           // 代币X程序ID
    );

    // 生成操作员接收代币Y的关联代币账户地址
    // 协议手续费中的代币Y部分将转入这个账户
    let receiver_token_y = get_associated_token_address_with_program_id(
        &program.payer(),                                           // 操作员地址
        &lb_pair_state.token_y_mint,                                // 代币Y铸造地址
        &token_y_program,                                           // 代币Y程序ID
    );

    // 生成协议手续费领取操作员的PDA
    // 这个账户存储了操作员的授权信息和限制条件
    let (claim_fee_operator, _) = derive_claim_protocol_fee_operator_pda(program.payer());

    // 构建提取协议手续费指令的主要账户列表
    let main_accounts = dlmm::client::accounts::WithdrawProtocolFee {
        lb_pair,                                                    // 流动性池对账户
        reserve_x: lb_pair_state.reserve_x,                         // 代币X储备金库账户
        reserve_y: lb_pair_state.reserve_y,                         // 代币Y储备金库账户
        token_x_mint: lb_pair_state.token_x_mint,                   // 代币X铸造地址
        token_y_mint: lb_pair_state.token_y_mint,                   // 代币Y铸造地址
        token_x_program,                                            // 代币X程序ID
        token_y_program,                                            // 代币Y程序ID
        receiver_token_x,                                           // 操作员代币X接收账户
        receiver_token_y,                                           // 操作员代币Y接收账户
        claim_fee_operator,                                         // 手续费领取操作员账户
        operator: program.payer(),                                  // 实际操作员账户
        memo_program: spl_memo::ID,                                 // 备注程序，用于记录操作日志
    }
    .to_account_metas(None);

    // 初始化额外账户信息和账户列表
    let mut remaining_accounts_info = RemainingAccountsInfo { slices: vec![] };
    let mut remaining_accounts = vec![];

    // 检查是否需要Token-2022相关的额外账户
    // 如果代币使用了转账钩子或其他Token-2022功能，需要额外账户
    if let Some((slices, transfer_hook_remaining_accounts)) =
        get_potential_token_2022_related_ix_data_and_accounts(
            &lb_pair_state,                                         // 池对状态信息
            program.rpc(),                                          // RPC客户端
            ActionType::Liquidity,                                  // 操作类型：流动性操作
        )
        .await?
    {
        remaining_accounts_info.slices = slices;
        remaining_accounts.extend(transfer_hook_remaining_accounts);
    };

    // 构建提取协议手续费指令的数据
    let data = dlmm::client::args::WithdrawProtocolFee {
        max_amount_x: u64::MAX,                                     // 代币X的最大提取数量（无限制）
        max_amount_y: u64::MAX,                                     // 代币Y的最大提取数量（无限制）
        remaining_accounts_info,                                    // 额外账户信息
    }
    .data();

    // 合并主要账户和额外账户列表
    let accounts = [main_accounts.to_vec(), remaining_accounts].concat();

    // 构建完整的提取协议手续费指令
    let withdraw_ix = Instruction {
        program_id: dlmm::ID,                                       // DLMM程序ID
        accounts,                                                   // 完整的账户列表
        data,                                                       // 指令数据
    };

    // 设置计算预算限制，由于涉及多个账户和复杂的Token-2022操作
    // 需要较高的计算单位来确保交易成功
    let compute_budget_ix = ComputeBudgetInstruction::set_compute_unit_limit(200_000);

    // 构建并发送交易请求
    let request_builder = program.request();
    let signature = request_builder
        .instruction(compute_budget_ix)                             // 先设置计算预算
        .instruction(withdraw_ix)                                   // 再添加提取指令
        .send_with_spinner_and_config(transaction_config)          // 发送交易并等待确认
        .await;

    println!("WithdrawProtocolFee. Signature: {:#?}", signature);

    // 检查交易是否成功执行
    signature?;

    Ok(())
}
