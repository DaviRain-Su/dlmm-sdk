use crate::*;
use anchor_spl::associated_token::get_associated_token_address;

/// 精确输出交易的参数结构体
/// Parameters for exact output swap
#[derive(Debug, Parser)]
pub struct SwapExactOutParams {
    /// 流动性交易对的地址
    /// Address of the liquidity pair.
    pub lb_pair: Pubkey,
    /// 要购买的代币数量
    /// Amount of token to be buy.
    pub amount_out: u64,
    /// 购买方向：true = 买入Y代币，false = 买入X代币
    /// Buy direction. true = buy token Y, false = buy token X.
    #[clap(long)]
    pub swap_for_y: bool,
}

/// 执行精确输出交易指令
/// Executes the exact output swap instruction
/// 
/// # 参数 / Parameters
/// * `params` - 精确输出交易的参数 / Parameters for exact output swap
/// * `program` - Solana程序引用 / Solana program reference
/// * `transaction_config` - 交易配置 / Transaction configuration
/// 
/// # 功能说明 / Functionality
/// 执行精确输出数量的交易，指定要获得的代币数量，系统计算需要支付的代币数量
/// Executes a swap with exact output amount, specifying the amount of tokens to receive,
/// and the system calculates the amount of tokens to pay
pub async fn execute_swap_exact_out<C: Deref<Target = impl Signer> + Clone>(
    params: SwapExactOutParams,
    program: &Program<C>,
    transaction_config: RpcSendTransactionConfig,
) -> Result<()> {
    let SwapExactOutParams {
        amount_out,
        lb_pair,
        swap_for_y,
    } = params;

    let rpc_client = program.rpc();
    
    // 获取流动性交易对状态数据
    // Get liquidity pair state data
    let lb_pair_state: LbPair = rpc_client
        .get_account_and_deserialize(&lb_pair, |account| {
            Ok(bytemuck::pod_read_unaligned(&account.data[8..]))
        })
        .await?;

    // 根据交易方向确定输入和输出代币账户
    // Determine input and output token accounts based on swap direction
    let (user_token_in, user_token_out) = if swap_for_y {
        // 用X代币买Y代币 / Use X token to buy Y token
        (
            get_associated_token_address(&program.payer(), &lb_pair_state.token_x_mint),
            get_associated_token_address(&program.payer(), &lb_pair_state.token_y_mint),
        )
    } else {
        // 用Y代币买X代币 / Use Y token to buy X token
        (
            get_associated_token_address(&program.payer(), &lb_pair_state.token_y_mint),
            get_associated_token_address(&program.payer(), &lb_pair_state.token_x_mint),
        )
    };

    // 派生bitmap扩展账户密钥
    // Derive bitmap extension account key
    let (bitmap_extension_key, _bump) = derive_bin_array_bitmap_extension(lb_pair);
    
    // 获取X和Y代币的程序ID
    // Get X and Y token program IDs
    let [token_x_program, token_y_program] = lb_pair_state.get_token_programs()?;

    // 尝试获取bitmap扩展账户（可能不存在）
    // Try to get bitmap extension account (may not exist)
    let bitmap_extension = rpc_client
        .get_account_and_deserialize(&bitmap_extension_key, |account| {
            Ok(bytemuck::pod_read_unaligned(&account.data[8..]))
        })
        .await
        .ok();

    // 获取交换所需的bin数组公钥
    // Get bin array public keys required for swap
    let bin_arrays_for_swap = get_bin_array_pubkeys_for_swap(
        lb_pair,
        &lb_pair_state,
        bitmap_extension.as_ref(),
        swap_for_y,
        3,  // 最多查找3个bin数组 / Search up to 3 bin arrays
    )?;

    // 获取报价所需的账户信息
    // Fetch accounts required for quote calculation
    let SwapQuoteAccounts {
        lb_pair_state,
        clock,
        mint_x_account,
        mint_y_account,
        bin_arrays,
        bin_array_keys,
    } = fetch_quote_required_accounts(&rpc_client, lb_pair, &lb_pair_state, bin_arrays_for_swap)
        .await?;

    // 计算精确输出交易的报价
    // Calculate quote for exact output swap
    let quote = quote_exact_out(
        lb_pair,
        &lb_pair_state,
        amount_out,            // 期望输出数量 / Desired output amount
        swap_for_y,
        bin_arrays,
        bitmap_extension.as_ref(),
        &clock,
        &mint_x_account,
        &mint_y_account,
    )?;

    // 派生事件权限PDA
    // Derive event authority PDA
    let (event_authority, _bump) = derive_event_authority_pda();

    // 构建主要账户信息
    // Build main account information
    let main_accounts = dlmm::client::accounts::SwapExactOut2 {
        lb_pair,                           // 流动性交易对 / Liquidity pair
        bin_array_bitmap_extension: bitmap_extension
            .map(|_| bitmap_extension_key)
            .or(Some(dlmm::ID)),           // Bitmap扩展账户或程序ID / Bitmap extension account or program ID
        reserve_x: lb_pair_state.reserve_x, // X代币储备账户 / X token reserve account
        reserve_y: lb_pair_state.reserve_y, // Y代币储备账户 / Y token reserve account
        token_x_mint: lb_pair_state.token_x_mint, // X代币铸币账户 / X token mint account
        token_y_mint: lb_pair_state.token_y_mint, // Y代币铸币账户 / Y token mint account
        token_x_program,                   // X代币程序 / X token program
        token_y_program,                   // Y代币程序 / Y token program
        user: program.payer(),             // 用户账户 / User account
        user_token_in,                     // 用户输入代币账户 / User input token account
        user_token_out,                    // 用户输出代币账户 / User output token account
        oracle: lb_pair_state.oracle,      // 预言机账户 / Oracle account
        host_fee_in: Some(dlmm::ID),       // 主机费用输入账户 / Host fee input account
        event_authority,                   // 事件权限 / Event authority
        program: dlmm::ID,                 // DLMM程序ID / DLMM program ID
        memo_program: spl_memo::ID,        // 备忘录程序 / Memo program
    }
    .to_account_metas(None);

    // 初始化剩余账户信息和Token 2022相关账户
    // Initialize remaining accounts info and Token 2022 related accounts
    let mut remaining_accounts_info = RemainingAccountsInfo { slices: vec![] };
    let mut remaining_accounts = vec![];

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
        remaining_accounts.extend(transfer_hook_remaining_accounts);
    }

    // 添加bin数组账户到剩余账户列表
    // Add bin array accounts to remaining accounts list
    remaining_accounts.extend(
        bin_array_keys
            .into_iter()
            .map(|key| AccountMeta::new(key, false)),
    );

    // 计算总输入金额（包含手续费）
    // Calculate total input amount (including fees)
    let in_amount = quote.amount_in + quote.fee;
    
    // 应用100个基点（1%）的滑点保护
    // Apply 100 basis points (1%) slippage protection
    let max_in_amount = in_amount * 10100 / BASIS_POINT_MAX as u64;

    // 构建交换指令数据
    // Build swap instruction data
    let data = dlmm::client::args::SwapExactOut2 {
        out_amount: amount_out,     // 精确输出数量 / Exact output amount
        max_in_amount,              // 最大输入数量 / Maximum input amount
        remaining_accounts_info,
    }
    .data();

    // 组合所有必需的账户
    // Combine all required accounts
    let accounts = [main_accounts.to_vec(), remaining_accounts].concat();

    // 创建交换指令
    // Create swap instruction
    let swap_ix = Instruction {
        program_id: dlmm::ID,
        accounts,
        data,
    };

    // 设置计算预算限制
    // Set compute budget limit
    let compute_budget_ix = ComputeBudgetInstruction::set_compute_unit_limit(1_400_000);

    // 构建并发送交易
    // Build and send transaction
    let request_builder = program.request();
    let signature = request_builder
        .instruction(compute_budget_ix)  // 添加计算预算指令 / Add compute budget instruction
        .instruction(swap_ix)            // 添加交换指令 / Add swap instruction
        .send_with_spinner_and_config(transaction_config)
        .await;

    println!("Swap. Signature: {:#?}", signature);

    signature?;

    Ok(())
}
