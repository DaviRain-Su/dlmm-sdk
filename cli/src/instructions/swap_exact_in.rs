use crate::*;
use anchor_spl::associated_token::get_associated_token_address_with_program_id;

/// 精确输入数量的交易参数
#[derive(Debug, Parser)]
pub struct SwapExactInParams {
    /// Address of the liquidity pair.
    /// 流动性对地址
    pub lb_pair: Pubkey,
    /// Amount of token to be sell.
    /// 要卖出的代币数量（精确输入）
    pub amount_in: u64,
    /// Buy direction. true = buy token Y, false = buy token X.
    /// 交易方向：true = 用X代币买Y代币，false = 用Y代币买X代币
    #[clap(long)]
    pub swap_for_y: bool,
}

/// 执行精确输入的交易
/// 
/// # 参数
/// * `params` - 交易参数
/// * `program` - Anchor程序客户端
/// * `transaction_config` - 交易配置
/// 
/// # 功能
/// 1. 获取流动性对状态
/// 2. 计算交易报价
/// 3. 构建并发送交易
pub async fn execute_swap<C: Deref<Target = impl Signer> + Clone>(
    params: SwapExactInParams,
    program: &Program<C>,
    transaction_config: RpcSendTransactionConfig,
) -> Result<()> {
    let SwapExactInParams {
        amount_in,
        lb_pair,
        swap_for_y,
    } = params;

    let rpc_client = program.rpc();

    // 获取流动性对的状态
    let lb_pair_state: LbPair = rpc_client
        .get_account_and_deserialize(&lb_pair, |account| {
            Ok(bytemuck::pod_read_unaligned(&account.data[8..]))
        })
        .await?;

    // 获取代币程序（支持Token和Token2022）
    let [token_x_program, token_y_program] = lb_pair_state.get_token_programs()?;

    // 根据交易方向确定输入和输出代币账户
    let (user_token_in, user_token_out) = if swap_for_y {
        (
            get_associated_token_address_with_program_id(
                &program.payer(),
                &lb_pair_state.token_x_mint,
                &token_x_program,
            ),
            get_associated_token_address_with_program_id(
                &program.payer(),
                &lb_pair_state.token_y_mint,
                &token_y_program,
            ),
        )
    } else {
        (
            get_associated_token_address_with_program_id(
                &program.payer(),
                &lb_pair_state.token_y_mint,
                &token_y_program,
            ),
            get_associated_token_address_with_program_id(
                &program.payer(),
                &lb_pair_state.token_x_mint,
                &token_x_program,
            ),
        )
    };

    // 获取bin数组位图扩展（用于优化bin查找）
    let (bitmap_extension_key, _bump) = derive_bin_array_bitmap_extension(lb_pair);

    let bitmap_extension = rpc_client
        .get_account_and_deserialize(&bitmap_extension_key, |account| {
            Ok(bytemuck::pod_read_unaligned(&account.data[8..]))
        })
        .await
        .ok();

    // 获取交易所需的bin数组公钥
    // 参数3表示获取3个bin数组，用于覆盖可能的交易范围
    let bin_arrays_for_swap = get_bin_array_pubkeys_for_swap(
        lb_pair,
        &lb_pair_state,
        bitmap_extension.as_ref(),
        swap_for_y,
        3,
    )?;

    let SwapQuoteAccounts {
        lb_pair_state,
        clock,
        mint_x_account,
        mint_y_account,
        bin_arrays,
        bin_array_keys,
    } = fetch_quote_required_accounts(&rpc_client, lb_pair, &lb_pair_state, bin_arrays_for_swap)
        .await?;

    let quote = quote_exact_in(
        lb_pair,
        &lb_pair_state,
        amount_in,
        swap_for_y,
        bin_arrays,
        bitmap_extension.as_ref(),
        &clock,
        &mint_x_account,
        &mint_y_account,
    )?;

    let (event_authority, _bump) = derive_event_authority_pda();

    let main_accounts = dlmm::client::accounts::Swap2 {
        lb_pair,
        bin_array_bitmap_extension: bitmap_extension
            .map(|_| bitmap_extension_key)
            .or(Some(dlmm::ID)),
        reserve_x: lb_pair_state.reserve_x,
        reserve_y: lb_pair_state.reserve_y,
        token_x_mint: lb_pair_state.token_x_mint,
        token_y_mint: lb_pair_state.token_y_mint,
        token_x_program,
        token_y_program,
        user: program.payer(),
        user_token_in,
        user_token_out,
        oracle: lb_pair_state.oracle,
        host_fee_in: Some(dlmm::ID),
        event_authority,
        program: dlmm::ID,
        memo_program: spl_memo::ID,
    }
    .to_account_metas(None);

    let mut remaining_accounts_info = RemainingAccountsInfo { slices: vec![] };
    let mut remaining_accounts = vec![];

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

    remaining_accounts.extend(
        bin_array_keys
            .into_iter()
            .map(|key| AccountMeta::new(key, false)),
    );

    // 100 bps slippage
    let min_amount_out = quote.amount_out * 9900 / BASIS_POINT_MAX as u64;

    let data = dlmm::client::args::Swap2 {
        amount_in,
        min_amount_out,
        remaining_accounts_info,
    }
    .data();

    let accounts = [main_accounts.to_vec(), remaining_accounts].concat();

    let swap_ix = Instruction {
        program_id: dlmm::ID,
        accounts,
        data,
    };

    let compute_budget_ix = ComputeBudgetInstruction::set_compute_unit_limit(1_400_000);

    let request_builder = program.request();
    let signature = request_builder
        .instruction(compute_budget_ix)
        .instruction(swap_ix)
        .send_with_spinner_and_config(transaction_config)
        .await;

    println!("Swap. Signature: {:#?}", signature);

    signature?;

    Ok(())
}
