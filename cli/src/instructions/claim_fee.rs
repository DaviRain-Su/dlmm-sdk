use crate::*;
use instructions::*;

/// 领取手续费的参数结构体
/// Parameters for claiming fees
#[derive(Debug, Parser)]
pub struct ClaimFeeParams {
    /// 仓位地址
    /// Position address
    pub position: Pubkey,
}

/// 执行领取手续费指令
/// Executes the claim fee instruction
/// 
/// # 参数 / Parameters
/// * `params` - 领取手续费的参数 / Parameters for fee claiming
/// * `program` - Solana程序引用 / Solana program reference
/// * `transaction_config` - 交易配置 / Transaction configuration
/// * `compute_unit_price` - 计算单元价格指令（可选）/ Compute unit price instruction (optional)
/// 
/// # 功能说明 / Functionality
/// 从指定的流动性仓位中领取累积的交易手续费到用户的代币账户
/// Claims accumulated trading fees from the specified liquidity position to user's token accounts
pub async fn execute_claim_fee<C: Deref<Target = impl Signer> + Clone>(
    params: ClaimFeeParams,
    program: &Program<C>,
    transaction_config: RpcSendTransactionConfig,
    compute_unit_price: Option<Instruction>,
) -> Result<()> {
    let ClaimFeeParams { position } = params;

    let rpc_client = program.rpc();
    
    // 获取仓位状态数据
    // Get position state data
    let position_state: PositionV2 = rpc_client
        .get_account_and_deserialize(&position, |account| {
            Ok(bytemuck::pod_read_unaligned(&account.data[8..]))
        })
        .await?;

    // 获取流动性交易对状态数据
    // Get liquidity pair state data
    let lb_pair_state: LbPair = rpc_client
        .get_account_and_deserialize(&position_state.lb_pair, |account| {
            Ok(bytemuck::pod_read_unaligned(&account.data[8..]))
        })
        .await?;

    // 确定手续费接收者并创建或获取相应的代币账户
    // Determine fee receiver and create or get corresponding token accounts
    let (user_token_x, user_token_y) = if position_state.fee_owner.eq(&Pubkey::default()) {
        // 如果没有指定手续费所有者，使用程序支付者
        // If no fee owner is specified, use program payer
        let user_token_x = get_or_create_ata(
            program,
            transaction_config,
            lb_pair_state.token_x_mint,
            program.payer(),
            compute_unit_price.clone(),
        )
        .await?;

        let user_token_y = get_or_create_ata(
            program,
            transaction_config,
            lb_pair_state.token_y_mint,
            program.payer(),
            compute_unit_price.clone(),
        )
        .await?;

        (user_token_x, user_token_y)
    } else {
        // 使用指定的手续费所有者
        // Use specified fee owner
        let user_token_x = get_or_create_ata(
            program,
            transaction_config,
            lb_pair_state.token_x_mint,
            position_state.fee_owner,
            compute_unit_price.clone(),
        )
        .await?;

        let user_token_y = get_or_create_ata(
            program,
            transaction_config,
            lb_pair_state.token_y_mint,
            position_state.fee_owner,
            compute_unit_price.clone(),
        )
        .await?;

        (user_token_x, user_token_y)
    };

    // 获取代币程序ID
    // Get token program IDs
    let [token_program_x, token_program_y] = lb_pair_state.get_token_programs()?;

    // 派生事件权限PDA
    // Derive event authority PDA
    let (event_authority, _bump) = derive_event_authority_pda();

    // 构建主要账户信息
    // Build main account information
    let main_accounts = dlmm::client::accounts::ClaimFee2 {
        lb_pair: position_state.lb_pair,        // 流动性交易对 / Liquidity pair
        sender: program.payer(),                // 发送者 / Sender
        position,                               // 仓位账户 / Position account
        reserve_x: lb_pair_state.reserve_x,     // X代币储备账户 / X token reserve account
        reserve_y: lb_pair_state.reserve_y,     // Y代币储备账户 / Y token reserve account
        token_program_x,                        // X代币程序 / X token program
        token_program_y,                        // Y代币程序 / Y token program
        token_x_mint: lb_pair_state.token_x_mint, // X代币铸币账户 / X token mint account
        token_y_mint: lb_pair_state.token_y_mint, // Y代币铸币账户 / Y token mint account
        user_token_x,                           // 用户X代币账户 / User X token account
        user_token_y,                           // 用户Y代币账户 / User Y token account
        event_authority,                        // 事件权限 / Event authority
        program: dlmm::ID,                      // DLMM程序ID / DLMM program ID
        memo_program: spl_memo::id(),           // 备忘录程序 / Memo program
    }
    .to_account_metas(None);

    // 初始化剩余账户信息和Token 2022相关账户
    // Initialize remaining accounts info and Token 2022 related accounts
    let mut remaining_accounts_info = RemainingAccountsInfo { slices: vec![] };
    let mut token_2022_remaining_accounts = vec![];

    // 获取可能的Token 2022相关指令数据和账户
    // Get potential Token 2022 related instruction data and accounts
    if let Some((slices, transfer_hook_remaining_accounts)) =
        get_potential_token_2022_related_ix_data_and_accounts(
            &lb_pair_state,
            program.rpc(),
            ActionType::Liquidity,
        )
        .await?
    {
        remaining_accounts_info.slices = slices;
        token_2022_remaining_accounts.extend(transfer_hook_remaining_accounts);
    };

    // 分块处理仓位的bin范围以领取手续费
    // Process position bin range in chunks to claim fees
    for (min_bin_id, max_bin_id) in
        position_bin_range_chunks(position_state.lower_bin_id, position_state.upper_bin_id)
    {
        // 构建领取手续费指令数据
        // Build claim fee instruction data
        let data = dlmm::client::args::ClaimFee2 {
            min_bin_id,     // 最小bin ID / Minimum bin ID
            max_bin_id,     // 最大bin ID / Maximum bin ID
            remaining_accounts_info: remaining_accounts_info.clone(),
        }
        .data();

        // 获取当前块覆盖的bin数组账户元数据
        // Get bin array account metadata covered by current chunk
        let bin_arrays_account_meta =
            position_state.get_bin_array_accounts_meta_coverage_by_chunk(min_bin_id, max_bin_id)?;

        // 组合所有必需的账户
        // Combine all required accounts
        let accounts = [
            main_accounts.to_vec(),
            token_2022_remaining_accounts.clone(),
            bin_arrays_account_meta,
        ]
        .concat();

        // 创建领取手续费指令
        // Create claim fee instruction
        let claim_fee_ix = Instruction {
            program_id: dlmm::ID,
            accounts,
            data,
        };

        // 构建交易请求
        // Build transaction request
        let mut request_builder = program.request();

        // 如果提供了计算单元价格指令，则添加
        // Add compute unit price instruction if provided
        if let Some(compute_unit_price_ix) = compute_unit_price.clone() {
            request_builder = request_builder.instruction(compute_unit_price_ix);
        }

        // 发送交易
        // Send transaction
        let signature = request_builder
            .instruction(claim_fee_ix)
            .send_with_spinner_and_config(transaction_config)
            .await;

        println!("Claim fee. Signature: {:#?}", signature);

        signature?;
    }

    Ok(())
}
