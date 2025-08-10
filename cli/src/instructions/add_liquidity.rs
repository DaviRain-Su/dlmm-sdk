use crate::*;
use commons::dlmm::accounts::{LbPair, PositionV2};
use instructions::*;

/// 添加流动性参数
#[derive(Debug, Parser)]
pub struct AddLiquidityParams {
    /// Address of the liquidity pair.
    /// 流动性对地址
    pub lb_pair: Pubkey,
    /// Position for the deposit.
    /// 用于存入流动性的仓位
    pub position: Pubkey,
    /// Amount of token X to be deposited.
    /// 要存入的X代币数量
    pub amount_x: u64,
    /// Amount of token Y to be deposited.
    /// 要存入的Y代币数量
    pub amount_y: u64,
    /// Liquidity distribution to the bins. "<DELTA_ID,DIST_X,DIST_Y, DELTA_ID,DIST_X,DIST_Y, ...>" where
    /// DELTA_ID = Number of bins surrounding the active bin. This decide which bin the token is going to deposit to. For example: if the current active id is 5555, delta_ids is 1, the user will be depositing to bin 5554, 5555, and 5556.
    /// DIST_X = Percentage of amount_x to be deposited to the bins. Must not > 1.0
    /// DIST_Y = Percentage of amount_y to be deposited to the bins. Must not > 1.0
    /// For example: --bin-liquidity-distribution "-1,0.0,0.25 0,0.75,0.75 1,0.25,0.0"
    /// 
    /// 流动性在各个bin中的分配。格式："<DELTA_ID,DIST_X,DIST_Y, ...>"
    /// DELTA_ID = 相对于当前活跃bin的偏移。例如：当前活跃ID是5555，delta_id为1表示存入bin 5556
    /// DIST_X = X代币分配到该bin的百分比（不得大于1.0）
    /// DIST_Y = Y代币分配到该bin的百分比（不得大于1.0）
    /// 示例：--bin-liquidity-distribution "-1,0.0,0.25 0,0.75,0.75 1,0.25,0.0"
    #[clap(long, value_parser = parse_bin_liquidity_distribution, value_delimiter = ' ', allow_hyphen_values = true)]
    pub bin_liquidity_distribution: Vec<(i32, f64, f64)>,
}

/// 执行添加流动性操作
/// 
/// # 参数
/// * `params` - 添加流动性参数
/// * `program` - Anchor程序客户端
/// * `transaction_config` - 交易配置
/// * `compute_unit_price` - 计算单元价格指令（可选）
/// 
/// # 功能
/// 1. 验证参数并排序流动性分配
/// 2. 获取必要的账户和状态
/// 3. 构建并发送添加流动性交易
pub async fn execute_add_liquidity<C: Deref<Target = impl Signer> + Clone>(
    params: AddLiquidityParams,
    program: &Program<C>,
    transaction_config: RpcSendTransactionConfig,
    compute_unit_price: Option<Instruction>,
) -> Result<()> {
    let AddLiquidityParams {
        lb_pair,
        position,
        amount_x,
        amount_y,
        mut bin_liquidity_distribution,
    } = params;

    // 按bin ID排序，确保从低到高
    bin_liquidity_distribution.sort_by(|a, b| a.0.cmp(&b.0));

    let rpc_client = program.rpc();

    // 获取流动性对状态
    let lb_pair_state: LbPair = rpc_client
        .get_account_and_deserialize(&lb_pair, |account| {
            Ok(bytemuck::pod_read_unaligned(&account.data[8..]))
        })
        .await?;

    // 获取代币程序（支持Token和Token2022）
    let [token_x_program, token_y_program] = lb_pair_state.get_token_programs()?;

    // 将百分比转换为基点（1 = 10000基点）
    let bin_liquidity_distribution = bin_liquidity_distribution
        .into_iter()
        .map(|(bin_id, dist_x, dist_y)| BinLiquidityDistribution {
            bin_id,
            distribution_x: (dist_x * BASIS_POINT_MAX as f64) as u16, // 转换为基点
            distribution_y: (dist_y * BASIS_POINT_MAX as f64) as u16, // 转换为基点
        })
        .collect::<Vec<_>>();

    // 获取仓位状态
    let position_state: PositionV2 = rpc_client
        .get_account_and_deserialize(&position, |account| {
            Ok(bytemuck::pod_read_unaligned(&account.data[8..]))
        })
        .await?;

    // 获取最小和最大bin ID，用于确定需要的bin数组范围
    let min_bin_id = bin_liquidity_distribution
        .first()
        .map(|bld| bld.bin_id)
        .context("No bin liquidity distribution provided")?;

    let max_bin_id = bin_liquidity_distribution
        .last()
        .map(|bld| bld.bin_id)
        .context("No bin liquidity distribution provided")?;

    // 获取覆盖所需bin范围的bin数组账户元数据
    let bin_arrays_account_meta =
        position_state.get_bin_array_accounts_meta_coverage_by_chunk(min_bin_id, max_bin_id)?;

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

    let (bin_array_bitmap_extension, _bump) = derive_bin_array_bitmap_extension(lb_pair);

    let bin_array_bitmap_extension = rpc_client
        .get_account(&bin_array_bitmap_extension)
        .await
        .map(|_| bin_array_bitmap_extension)
        .ok()
        .or(Some(dlmm::ID));

    let (event_authority, _bump) = derive_event_authority_pda();

    let main_accounts = dlmm::client::accounts::AddLiquidity2 {
        lb_pair,
        bin_array_bitmap_extension,
        position,
        reserve_x: lb_pair_state.reserve_x,
        reserve_y: lb_pair_state.reserve_y,
        token_x_mint: lb_pair_state.token_x_mint,
        token_y_mint: lb_pair_state.token_y_mint,
        sender: program.payer(),
        user_token_x,
        user_token_y,
        token_x_program,
        token_y_program,
        event_authority,
        program: dlmm::ID,
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
    };

    remaining_accounts.extend(bin_arrays_account_meta);

    let data = dlmm::client::args::AddLiquidity2 {
        liquidity_parameter: LiquidityParameter {
            amount_x,
            amount_y,
            bin_liquidity_dist: bin_liquidity_distribution,
        },
        remaining_accounts_info,
    }
    .data();

    let accounts = [main_accounts.to_vec(), remaining_accounts].concat();

    let add_liquidity_ix = Instruction {
        program_id: dlmm::ID,
        accounts,
        data,
    };

    let compute_budget_ix = ComputeBudgetInstruction::set_compute_unit_limit(1_400_000);

    let request_builder = program.request();
    let signature = request_builder
        .instruction(compute_budget_ix)
        .instruction(add_liquidity_ix)
        .send_with_spinner_and_config(transaction_config)
        .await;

    println!("Add Liquidity. Signature: {:#?}", signature);

    signature?;

    Ok(())
}
