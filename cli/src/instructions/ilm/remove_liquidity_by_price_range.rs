use crate::*;
use anchor_lang::AccountDeserialize;
use anchor_spl::token_interface::Mint;
use instructions::*;

/// 按价格范围移除流动性的参数结构体
/// Parameters for removing liquidity by price range
#[derive(Debug, Parser)]
pub struct RemoveLiquidityByPriceRangeParameters {
    /// 流动性对的地址 / Address of the liquidity pair
    pub lb_pair: Pubkey,
    /// 基础头寸密钥 / Base position key
    pub base_position_key: Pubkey,
    /// 最小价格 / Minimum price
    pub min_price: f64,
    /// 最大价格 / Maximum price
    pub max_price: f64,
}

/// 执行按价格范围移除流动性
/// Execute removing liquidity by price range
pub async fn execute_remove_liquidity_by_price_range<C: Deref<Target = impl Signer> + Clone>(
    params: RemoveLiquidityByPriceRangeParameters,
    program: &Program<C>,
    transaction_config: RpcSendTransactionConfig,
    compute_unit_price: Option<Instruction>,
) -> Result<()> {
    // 解构参数
    // Destructure parameters
    let RemoveLiquidityByPriceRangeParameters {
        lb_pair,
        base_position_key,
        min_price,
        max_price,
    } = params;

    let rpc_client = program.rpc();

    // 获取流动性对状态
    // Get liquidity pair state
    let lb_pair_state: LbPair = rpc_client
        .get_account_and_deserialize(&lb_pair, |account| {
            Ok(bytemuck::pod_read_unaligned(&account.data[8..]))
        })
        .await?;

    // 获取bin步长和代币程序
    // Get bin step and token programs
    let bin_step = lb_pair_state.bin_step;
    let [token_x_program, token_y_program] = lb_pair_state.get_token_programs()?;

    // 获取代币铸币账户信息
    // Get token mint account information
    let mut accounts = rpc_client
        .get_multiple_accounts(&[lb_pair_state.token_x_mint, lb_pair_state.token_y_mint])
        .await?;

    let token_mint_base_account = accounts[0].take().context("token_mint_base not found")?;
    let token_mint_quote_account = accounts[1].take().context("token_mint_quote not found")?;

    let token_mint_base = Mint::try_deserialize(&mut token_mint_base_account.data.as_ref())?;
    let token_mint_quote = Mint::try_deserialize(&mut token_mint_quote_account.data.as_ref())?;

    // 将最小价格转换为每lamport价格并计算对应的bin ID
    // Convert min price to per-lamport price and calculate corresponding bin ID
    let min_price_per_lamport = price_per_token_to_per_lamport(
        min_price,
        token_mint_base.decimals,
        token_mint_quote.decimals,
    )
    .context("price_per_token_to_per_lamport overflow")?;

    let min_active_id = get_id_from_price(bin_step, &min_price_per_lamport, Rounding::Up)
        .context("get_id_from_price overflow")?;

    // 将最大价格转换为每lamport价格并计算对应的bin ID
    // Convert max price to per-lamport price and calculate corresponding bin ID
    let max_price_per_lamport = price_per_token_to_per_lamport(
        max_price,
        token_mint_base.decimals,
        token_mint_quote.decimals,
    )
    .context("price_per_token_to_per_lamport overflow")?;

    let max_active_id = get_id_from_price(bin_step, &max_price_per_lamport, Rounding::Up)
        .context("get_id_from_price overflow")?;

    // 验证价格范围有效
    // Verify price range is valid
    assert!(min_active_id < max_active_id);

    // 获取或创建用户的X代币账户
    // Get or create user's X token account
    let user_token_x = get_or_create_ata(
        program,
        transaction_config,
        lb_pair_state.token_x_mint,
        program.payer(),
        compute_unit_price.clone(),
    )
    .await?;

    // 获取或创建用户的Y代币账户
    // Get or create user's Y token account
    let user_token_y = get_or_create_ata(
        program,
        transaction_config,
        lb_pair_state.token_y_mint,
        program.payer(),
        compute_unit_price.clone(),
    )
    .await?;

    let (event_authority, _bump) = derive_event_authority_pda();

    let (bin_array_bitmap_extension, _bump) = derive_bin_array_bitmap_extension(lb_pair);
    let bin_array_bitmap_extension = rpc_client
        .get_account(&bin_array_bitmap_extension)
        .await
        .map(|_| bin_array_bitmap_extension)
        .ok()
        .or(Some(dlmm::ID));

    let width = DEFAULT_BIN_PER_POSITION as i32;

    let mut remaining_accounts_info = RemainingAccountsInfo { slices: vec![] };
    let mut transfer_hook_remaining_accounts = vec![];

    if let Some((slices, remaining_accounts)) =
        get_potential_token_2022_related_ix_data_and_accounts(
            &lb_pair_state,
            program.rpc(),
            ActionType::Liquidity,
        )
        .await?
    {
        remaining_accounts_info.slices = slices;
        transfer_hook_remaining_accounts.extend(remaining_accounts);
    };

    // 遍历价格范围内的所有bin ID
    // Iterate through all bin IDs in the price range
    for i in min_active_id..=max_active_id {
        // 派生头寸PDA地址
        // Derive position PDA address
        let (position, _bump) = derive_position_pda(lb_pair, base_position_key, i, width);

        // 获取头寸账户
        // Get position account
        let position_account = rpc_client.get_account(&position).await;
        if let std::result::Result::Ok(account) = position_account {
            // 解析头寸状态
            // Parse position state
            let position_state: PositionV2 = bytemuck::pod_read_unaligned(&account.data[8..]);

            let bin_arrays_account_meta = position_state.get_bin_array_accounts_meta_coverage()?;

            let remaining_accounts = [
                transfer_hook_remaining_accounts.clone(),
                bin_arrays_account_meta,
            ]
            .concat();

            // 设置计算单元限制
            // Set compute unit limit
            let mut instructions =
                vec![ComputeBudgetInstruction::set_compute_unit_limit(1_400_000)];

            // 创建移除流动性指令
            // Create remove liquidity instruction
            let main_accounts = dlmm::client::accounts::RemoveLiquidityByRange2 {
                position,
                lb_pair,
                bin_array_bitmap_extension,
                user_token_x,
                user_token_y,
                reserve_x: lb_pair_state.reserve_x,
                reserve_y: lb_pair_state.reserve_y,
                token_x_mint: lb_pair_state.token_x_mint,
                token_y_mint: lb_pair_state.token_y_mint,
                sender: program.payer(),
                token_x_program,
                token_y_program,
                memo_program: spl_memo::ID,
                event_authority,
                program: dlmm::ID,
            }
            .to_account_metas(None);

            let data = dlmm::client::args::RemoveLiquidityByRange2 {
                from_bin_id: position_state.lower_bin_id,
                to_bin_id: position_state.upper_bin_id,
                bps_to_remove: BASIS_POINT_MAX as u16,
                remaining_accounts_info: remaining_accounts_info.clone(),
            }
            .data();

            let accounts = [main_accounts.to_vec(), remaining_accounts.clone()].concat();

            let withdraw_all_ix = Instruction {
                program_id: dlmm::ID,
                accounts,
                data,
            };

            instructions.push(withdraw_all_ix);

            // 创建申领费用指令
            // Create claim fee instruction
            let main_accounts = dlmm::client::accounts::ClaimFee2 {
                lb_pair,
                position,
                sender: program.payer(),
                reserve_x: lb_pair_state.reserve_x,
                reserve_y: lb_pair_state.reserve_y,
                token_x_mint: lb_pair_state.token_x_mint,
                token_y_mint: lb_pair_state.token_y_mint,
                token_program_x: token_x_program,
                token_program_y: token_y_program,
                memo_program: spl_memo::ID,
                event_authority,
                program: dlmm::ID,
                user_token_x,
                user_token_y,
            }
            .to_account_metas(None);

            let data = dlmm::client::args::ClaimFee2 {
                min_bin_id: position_state.lower_bin_id,
                max_bin_id: position_state.upper_bin_id,
                remaining_accounts_info: remaining_accounts_info.clone(),
            }
            .data();

            let accounts = [main_accounts.to_vec(), remaining_accounts.clone()].concat();

            let claim_fee_ix = Instruction {
                program_id: dlmm::ID,
                accounts,
                data,
            };

            instructions.push(claim_fee_ix);

            // 创建关闭头寸指令
            // Create close position instruction
            let accounts = dlmm::client::accounts::ClosePosition2 {
                position,
                sender: program.payer(),
                rent_receiver: program.payer(),
                event_authority,
                program: dlmm::ID,
            }
            .to_account_metas(None);

            let data = dlmm::client::args::ClosePosition2 {}.data();

            let close_position_ix = Instruction {
                program_id: dlmm::ID,
                accounts,
                data,
            };

            instructions.push(close_position_ix);

            // 打印关闭头寸信息
            // Print position closing information
            println!(
                "Close position {}. Min bin id {}, Max bin id {}",
                position, position_state.lower_bin_id, position_state.upper_bin_id
            );
        }
    }
    Ok(())
}
