use crate::*;
use anchor_spl::token_interface::Mint;

/// 同步价格的参数结构体
/// Parameters for syncing price
#[derive(Debug, Parser)]
pub struct SyncPriceParams {
    /// 流动性交易对地址
    /// Liquidity pair address
    pub lb_pair: Pubkey,
    /// 目标价格
    /// Target price
    pub price: f64,
}

/// 执行同步价格指令
/// Executes the sync price instruction
/// 
/// # 参数 / Parameters
/// * `params` - 同步价格的参数 / Parameters for price synchronization
/// * `program` - Solana程序引用 / Solana program reference
/// * `transaction_config` - 交易配置 / Transaction configuration
/// * `compute_unit_price` - 计算单元价格指令（可选）/ Compute unit price instruction (optional)
/// 
/// # 功能说明 / Functionality
/// 将流动性交易对的活跃价格同步到指定的目标价格
/// Synchronizes the active price of the liquidity pair to the specified target price
pub async fn execute_sync_price<C: Deref<Target = impl Signer> + Clone>(
    params: SyncPriceParams,
    program: &Program<C>,
    transaction_config: RpcSendTransactionConfig,
    compute_unit_price: Option<Instruction>,
) -> Result<()> {
    let SyncPriceParams { lb_pair, price } = params;

    let rpc_client = program.rpc();

    // 派生bin数组bitmap扩展账户
    // Derive bin array bitmap extension account
    let (bin_array_bitmap_extension, _bump) = derive_bin_array_bitmap_extension(lb_pair);

    // 获取流动性交易对状态数据
    // Get liquidity pair state data
    let lb_pair_state: LbPair = rpc_client
        .get_account_and_deserialize(&lb_pair, |account| {
            Ok(bytemuck::pod_read_unaligned(&account.data[8..]))
        })
        .await?;

    // 获取多个账户信息：代币铸币和bitmap扩展账户
    // Get multiple account information: token mints and bitmap extension account
    let mut accounts = rpc_client
        .get_multiple_accounts(&[
            lb_pair_state.token_x_mint,     // X代币铸币账户 / X token mint account
            lb_pair_state.token_y_mint,     // Y代币铸币账户 / Y token mint account
            bin_array_bitmap_extension,     // bin数组bitmap扩展账户 / Bin array bitmap extension account
        ])
        .await?;

    let token_mint_base_account = accounts[0].take().context("token_mint_base not found")?;
    let token_mint_quote_account = accounts[1].take().context("token_mint_quote not found")?;
    let bin_array_bitmap_extension_account = accounts[2].take();

    // 反序列化代币铸币数据
    // Deserialize token mint data
    let token_mint_base = Mint::try_deserialize(&mut token_mint_base_account.data.as_ref())?;
    let token_mint_quote = Mint::try_deserialize(&mut token_mint_quote_account.data.as_ref())?

    // 将每代币价格转换为每单位最小代币价格（考虑小数位数）
    // Convert per-token price to per-lamport price (considering decimals)
    let price_per_lamport =
        price_per_token_to_per_lamport(price, token_mint_base.decimals, token_mint_quote.decimals)
            .context("price_per_token_to_per_lamport overflow")?;

    // 从价格计算对应的活跃bin ID
    // Calculate corresponding active bin ID from price
    let computed_active_id =
        get_id_from_price(lb_pair_state.bin_step, &price_per_lamport, Rounding::Up)
            .context("get_id_from_price overflow")?

    // 构建“跳转到指定bin”指令数据
    // Build "go to a bin" instruction data
    let ix_data = dlmm::client::args::GoToABin {
        bin_id: computed_active_id,  // 目标bin ID / Target bin ID
    }
    .data();

    // 计算当前和目标bin所在的bin数组索引
    // Calculate bin array indices for current and target bins
    let from_bin_array_idx = BinArray::bin_id_to_bin_array_index(lb_pair_state.active_id)?;
    let to_bin_array_idx = BinArray::bin_id_to_bin_array_index(computed_active_id)?;

    // 派生当前和目标bin数组的PDA地址
    // Derive PDA addresses for current and target bin arrays
    let (from_bin_array, _bump) = derive_bin_array_pda(lb_pair, from_bin_array_idx.into());
    let (to_bin_array, _bump) = derive_bin_array_pda(lb_pair, to_bin_array_idx.into());

    // 获取当前和目标bin数组账户
    // Get current and target bin array accounts
    accounts = rpc_client
        .get_multiple_accounts(&[from_bin_array, to_bin_array])
        .await?;

    let from_bin_array_account = accounts[0].take();
    let to_bin_array_account = accounts[1].take();

    // 派生事件权限PDA
    // Derive event authority PDA
    let (event_authority, _bump) = derive_event_authority_pda();

    // 构建跳转指令所需的账户信息
    // Build account information required for go-to instruction
    let accounts = dlmm::client::accounts::GoToABin {
        lb_pair,                        // 流动性交易对 / Liquidity pair
        bin_array_bitmap_extension: bin_array_bitmap_extension_account
            .map(|_| bin_array_bitmap_extension)
            .or(Some(dlmm::ID)),        // Bitmap扩展账户或程序ID / Bitmap extension account or program ID
        from_bin_array: from_bin_array_account
            .map(|_| from_bin_array)
            .or(Some(dlmm::ID)),        // 来源bin数组账户或程序ID / Source bin array account or program ID
        to_bin_array: to_bin_array_account
            .map(|_| to_bin_array)
            .or(Some(dlmm::ID)),        // 目标bin数组账户或程序ID / Target bin array account or program ID
        event_authority,                // 事件权限 / Event authority
        program: dlmm::ID,              // DLMM程序ID / DLMM program ID
    }
    .to_account_metas(None);

    // 创建跳转指令
    // Create go-to instruction
    let ix = Instruction {
        program_id: dlmm::ID,
        accounts,
        data: ix_data,
    };

    // 构建指令列表
    // Build instruction list
    let mut ixs = vec![];

    // 如果提供了计算单元价格指令，先添加它
    // Add compute unit price instruction first if provided
    if let Some(compute_unit_price_ix) = compute_unit_price {
        ixs.push(compute_unit_price_ix);
    }

    // 添加价格同步指令
    // Add price sync instruction
    ixs.push(ix);

    // 构建交易并发送
    // Build transaction and send
    let builder = program.request();
    let builder = ixs
        .into_iter()
        .fold(builder, |builder, ix| builder.instruction(ix));

    let signature = builder
        .send_with_spinner_and_config(transaction_config)
        .await;
    println!("{:#?}", signature);

    signature?;

    Ok()
}
