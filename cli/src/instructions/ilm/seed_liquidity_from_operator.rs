use std::{collections::HashMap, ops::Index, u64};

use crate::*;
use anchor_lang::{prelude::Clock, AccountDeserialize};
use anchor_spl::{
    associated_token::get_associated_token_address_with_program_id,
    token_interface::{spl_token_2022::instruction::transfer_checked, Mint, TokenAccount},
};

use futures_util::future::try_join_all;
use spl_associated_token_account::instruction::create_associated_token_account_idempotent;

/// 将代币数量转换为最小单位（Wei）
/// Convert token amount to smallest unit (Wei)
pub fn to_wei_amount(amount: u64, decimal: u8) -> Result<u64> {
    // 乘以10的小数位数次方来转换为最小单位
    // Multiply by 10^decimal to convert to smallest unit
    let wei_amount = amount
        .checked_mul(10u64.pow(decimal.into()))
        .context("to_wei_amount overflow")?;

    Ok(wei_amount)
}

/// 将用户界面价格范围转换为相应的bin ID范围
/// Convert UI price range to corresponding bin ID range
pub fn convert_min_max_ui_price_to_min_max_bin_id(
    bin_step: u16,
    min_price: f64,
    max_price: f64,
    base_token_decimal: u8,
    quote_token_decimal: u8,
) -> Result<(i32, i32)> {
    // 将最小价格转换为每lamport价格
    // Convert min price to per-lamport price
    let min_price_per_lamport =
        price_per_token_to_per_lamport(min_price, base_token_decimal, quote_token_decimal)
            .context("price_per_token_to_per_lamport overflow")?;

    // 从价格获取最小活跃ID
    // Get min active ID from price
    let min_active_id = get_id_from_price(bin_step, &min_price_per_lamport, Rounding::Up)
        .context("get_id_from_price overflow")?;

    // 将最大价格转换为每lamport价格
    // Convert max price to per-lamport price
    let max_price_per_lamport =
        price_per_token_to_per_lamport(max_price, base_token_decimal, quote_token_decimal)
            .context("price_per_token_to_per_lamport overflow")?;

    // 从价格获取最大活跃ID
    // Get max active ID from price
    let max_active_id = get_id_from_price(bin_step, &max_price_per_lamport, Rounding::Up)
        .context("get_id_from_price overflow")?;

    Ok((min_active_id, max_active_id))
}

/// 根据bin步长获取基础值
/// Get base value from bin step
fn get_base(bin_step: u16) -> f64 {
    // 基础值 = 1 + bin_step/10000, 用于价格计算
    // Base value = 1 + bin_step/10000, used for price calculations
    1.0 + bin_step as f64 / 10_000.0
}

/// 从bin ID获取用户界面价格
/// Get UI price from bin ID
pub fn get_ui_price_from_id(
    bin_step: u16,
    bin_id: i32,
    base_token_decimal: i32,
    quote_token_decimal: i32,
) -> f64 {
    let base = get_base(bin_step);
    // 价格 = 基础值^bin_id * 10^(基础代币小数位-报价代币小数位)
    // Price = base^bin_id * 10^(base_token_decimal - quote_token_decimal)
    base.powi(bin_id) * 10.0f64.powi(base_token_decimal - quote_token_decimal)
}

/// 获取覆盖指定范围所需的头寸数量
/// Get number of positions required to cover the specified range
pub fn get_number_of_position_required_to_cover_range(
    min_bin_id: i32,
    max_bin_id: i32,
) -> Result<i32> {
    // 计算bin ID的差值
    // Calculate the difference between bin IDs
    let bin_delta = max_bin_id
        .checked_sub(min_bin_id)
        .context("bin_delta overflow")?;
        
    // 计算需要的头寸数量（每个头寸包含DEFAULT_BIN_PER_POSITION个bin）
    // Calculate required positions (each position contains DEFAULT_BIN_PER_POSITION bins)
    let mut position_required = bin_delta
        .checked_div(DEFAULT_BIN_PER_POSITION as i32)
        .context("position_required overflow")?;
        
    // 如果有余数，需要额外的头寸
    // If there's a remainder, need an additional position
    let rem = bin_delta % DEFAULT_BIN_PER_POSITION as i32;

    if rem > 0 {
        position_required += 1;
    }

    Ok(position_required)
}

/// 压缩结果结构体
/// Compression result structure
struct CompressionResult {
    /// 压缩后的bin数量映射 / Compressed bin amount mapping
    compressed_bin_amount: HashMap<i32, u32>,
    /// 压缩损失 / Compression loss
    compression_loss: u64,
}

/// 压缩bin数量以适应存储限制
/// Compress bin amounts to fit storage limitations
fn compress_bin_amount(
    bins_amount: HashMap<i32, u64>,
    multiplier: u64,
) -> Result<CompressionResult> {
    let mut compressed_bin_amount = HashMap::new();
    let mut compression_loss = 0u64;

    // 遍历每个bin的数量并进行压缩
    // Iterate through each bin amount and compress
    for (bin_id, amount) in bins_amount.into_iter() {
        // 通过除法压缩数量到u32范围
        // Compress amount to u32 range by division
        let compressed_amount: u32 = amount
            .checked_div(multiplier)
            .context("overflow")?
            .try_into()
            .context("compressed fail")?;
        compressed_bin_amount.insert(bin_id, compressed_amount);

        // 计算压缩损失
        // Calculate compression loss
        let loss = amount
            .checked_sub(
                u64::from(compressed_amount)
                    .checked_mul(multiplier)
                    .context("overflow")?,
            )
            .context("overflow")?;

        compression_loss = compression_loss.checked_add(loss).context("overflow")?;
    }

    Ok(CompressionResult {
        compressed_bin_amount,
        compression_loss,
    })
}

/// 操作员播种流动性的参数结构体
/// Seed liquidity by operator parameters structure
#[derive(Debug, Parser, Clone)]
pub struct SeedLiquidityByOperatorParameters {
    /// 流动性对的地址 / Address of the liquidity pair
    #[clap(long)]
    pub lb_pair: Pubkey,
    /// 基础头寸路径 / Base position path
    #[clap(long)]
    pub base_position_path: String,
    /// X代币的数量 / Amount of X token
    #[clap(long)]
    pub amount: u64,
    /// 最小价格 / Minimum price
    #[clap(long)]
    pub min_price: f64,
    /// 最大价格 / Maximum price
    #[clap(long)]
    pub max_price: f64,
    /// 基础公钥 / Base public key
    #[clap(long)]
    pub base_pubkey: Pubkey,
    /// 曲率参数 / Curvature parameter
    #[clap(long)]
    pub curvature: f64,
    /// 头寸所有者 / Position owner
    #[clap(long)]
    pub position_owner: Pubkey,
    /// 费用所有者 / Fee owner
    #[clap(long)]
    pub fee_owner: Pubkey,
    /// 锁定释放点 / Lock release point
    #[clap(long)]
    pub lock_release_point: u64,
    /// 最大重试次数 / Maximum retries
    #[clap(long)]
    pub max_retries: u16,
}

/// 执行操作员播种流动性
/// Execute seed liquidity by operator
pub async fn execute_seed_liquidity_by_operator<C: Deref<Target = impl Signer> + Clone>(
    params: SeedLiquidityByOperatorParameters,
    program: &Program<C>,
    transaction_config: RpcSendTransactionConfig,
    compute_unit_price: Option<Instruction>,
) -> Result<()> {
    // 解构参数
    // Destructure parameters
    let SeedLiquidityByOperatorParameters {
        lb_pair,
        base_position_path,
        amount,
        min_price,
        max_price,
        base_pubkey,
        curvature,
        position_owner,
        fee_owner,
        lock_release_point,
        ..
    } = params;

    // 读取头寸基础密钥对文件
    // Read position base keypair file
    let position_base_kp = read_keypair_file(base_position_path.clone())
        .expect("position base keypair file not found");

    // 验证基础公钥是否匹配
    // Verify base public key matches
    assert!(
        position_base_kp.pubkey() == base_pubkey,
        "base_pubkey mismatch"
    );

    let rpc_client = program.rpc();

    // 计算k值（曲率的倒数）用于流动性分布
    // Calculate k value (reciprocal of curvature) for liquidity distribution
    let k = 1.0 / curvature;

    // 获取流动性对状态
    // Get liquidity pair state
    let lb_pair_state: LbPair = rpc_client
        .get_account_and_deserialize(&lb_pair, |account| {
            Ok(bytemuck::pod_read_unaligned(&account.data[8..]))
        })
        .await?;

    // 获取bin步长
    // Get bin step
    let bin_step = lb_pair_state.bin_step;

    let (mut bitmap_extension, _bump) = derive_bin_array_bitmap_extension(lb_pair);

    let mut accounts = rpc_client
        .get_multiple_accounts(&[
            lb_pair_state.token_x_mint,
            lb_pair_state.token_y_mint,
            solana_sdk::sysvar::clock::ID,
            bitmap_extension,
        ])
        .await?;

    let token_mint_base_account = accounts[0].take().context("token_mint_base not found")?;
    let token_mint_quote_account = accounts[1].take().context("token_mint_quote not found")?;
    let clock_account = accounts[2].take().context("clock not found")?;
    let bitmap_extension_account = accounts[3].take();

    let token_mint_base = Mint::try_deserialize(&mut token_mint_base_account.data.as_ref())?;
    let token_mint_quote = Mint::try_deserialize(&mut token_mint_quote_account.data.as_ref())?;
    let clock = bincode::deserialize::<Clock>(&clock_account.data)?;

    let fund_amount = to_wei_amount(amount, token_mint_base.decimals)?;

    let (min_bin_id, max_bin_id) = convert_min_max_ui_price_to_min_max_bin_id(
        bin_step,
        min_price,
        max_price,
        token_mint_base.decimals,
        token_mint_quote.decimals,
    )?;

    let actual_min_price = get_ui_price_from_id(
        bin_step,
        min_bin_id,
        token_mint_base.decimals.into(),
        token_mint_quote.decimals.into(),
    );
    let actual_max_price = get_ui_price_from_id(
        bin_step,
        max_bin_id,
        token_mint_base.decimals.into(),
        token_mint_quote.decimals.into(),
    );

    let position_number = get_number_of_position_required_to_cover_range(min_bin_id, max_bin_id)?;

    println!("Start seed. Min price: {} Max price: {} Actual min price: {} Actual max price: {} Min bin id: {} Max bin id: {} Position: {}", min_price, max_price, actual_min_price, actual_max_price, min_bin_id, max_bin_id, position_number);

    assert!(min_bin_id < max_bin_id, "Invalid price range");

    let bins_amount = generate_amount_for_bins(
        bin_step,
        min_bin_id,
        max_bin_id,
        actual_min_price,
        actual_max_price,
        token_mint_base.decimals,
        token_mint_quote.decimals,
        fund_amount,
        k,
    );

    let bins_amount_map: HashMap<i32, u64> = bins_amount
        .iter()
        .map(|(bin_id, amount_x)| (*bin_id, *amount_x))
        .collect();

    let decompress_multiplier = 10u64.pow(token_mint_base.decimals.into());

    let CompressionResult {
        compressed_bin_amount,
        compression_loss,
    } = compress_bin_amount(bins_amount_map, decompress_multiplier)?;

    let width = DEFAULT_BIN_PER_POSITION as i32;

    let mut token_account_and_bitmap_ext_and_token_prove_setup_ixs = vec![];
    let mut position_and_bin_array_setup_ixs = vec![];
    let mut liquidity_setup_ixs = vec![];

    let (event_authority, _bump) = derive_event_authority_pda();
    let seeder = program.payer();

    let token_mint_base_owner = token_mint_base_account.owner;
    let token_mint_quote_owner = token_mint_quote_account.owner;

    let seeder_token_x = get_associated_token_address_with_program_id(
        &seeder,
        &lb_pair_state.token_x_mint,
        &token_mint_base_owner,
    );

    let seeder_token_y = get_associated_token_address_with_program_id(
        &seeder,
        &lb_pair_state.token_y_mint,
        &token_mint_quote_owner,
    );

    let owner_token_x = get_associated_token_address_with_program_id(
        &position_owner,
        &lb_pair_state.token_x_mint,
        &token_mint_base_owner,
    );

    let transfer_hook_x_account =
        get_extra_account_metas_for_transfer_hook(lb_pair_state.token_x_mint, program.rpc())
            .await?;

    let transfer_hook_y_account =
        get_extra_account_metas_for_transfer_hook(lb_pair_state.token_y_mint, program.rpc())
            .await?;

    let accounts = rpc_client
        .get_multiple_accounts(&[seeder_token_x, seeder_token_y, owner_token_x])
        .await?;

    let seeder_token_x_account = accounts.index(0);
    if seeder_token_x_account.is_none() {
        token_account_and_bitmap_ext_and_token_prove_setup_ixs.push(
            create_associated_token_account_idempotent(
                &seeder,
                &seeder,
                &lb_pair_state.token_x_mint,
                &token_mint_base_owner,
            ),
        );
    }

    let seeder_token_y_account = accounts.index(1);
    if seeder_token_y_account.is_none() {
        token_account_and_bitmap_ext_and_token_prove_setup_ixs.push(
            create_associated_token_account_idempotent(
                &seeder,
                &seeder,
                &lb_pair_state.token_y_mint,
                &token_mint_quote_owner,
            ),
        );
    }

    let owner_token_x_account = accounts.index(2);
    let mut require_token_prove = false;

    if owner_token_x_account.is_none() {
        require_token_prove = true;
    } else if let Some(account) = owner_token_x_account.to_owned() {
        let owner_token_x_state = TokenAccount::try_deserialize(&mut account.data.as_slice())?;
        require_token_prove = owner_token_x_state.amount == 0;
    }

    if require_token_prove {
        token_account_and_bitmap_ext_and_token_prove_setup_ixs.push(
            create_associated_token_account_idempotent(
                &seeder,
                &position_owner,
                &lb_pair_state.token_x_mint,
                &token_mint_base_owner,
            ),
        );

        let prove_amount =
            calculate_transfer_fee_included_amount(&token_mint_base_account, 1, clock.epoch)?
                .amount;

        let mut transfer_ix = transfer_checked(
            &token_mint_base_owner,
            &seeder_token_x,
            &lb_pair_state.token_x_mint,
            &owner_token_x,
            &seeder,
            &[],
            prove_amount,
            token_mint_base.decimals,
        )?;

        transfer_ix
            .accounts
            .extend_from_slice(&transfer_hook_x_account);

        token_account_and_bitmap_ext_and_token_prove_setup_ixs.push(transfer_ix);
    }

    let (min_bitmap_id, max_bitmap_id) = LbPair::bitmap_range();
    let lower_bin_array_index = BinArray::bin_id_to_bin_array_index(min_bin_id)?;
    let upper_bin_array_index = BinArray::bin_id_to_bin_array_index(max_bin_id - 1)?;

    let overflow_internal_bitmap_range =
        upper_bin_array_index > max_bitmap_id || lower_bin_array_index < min_bitmap_id;

    if overflow_internal_bitmap_range && bitmap_extension_account.is_none() {
        let accounts = dlmm::client::accounts::InitializeBinArrayBitmapExtension {
            lb_pair,
            bin_array_bitmap_extension: bitmap_extension,
            funder: seeder,
            system_program: solana_sdk::system_program::ID,
            rent: solana_sdk::sysvar::rent::ID,
        }
        .to_account_metas(None);

        let ix_data = dlmm::client::args::InitializeBinArrayBitmapExtension {}.data();

        let init_bitmap_ext_ix = Instruction {
            program_id: dlmm::ID,
            accounts,
            data: ix_data,
        };

        token_account_and_bitmap_ext_and_token_prove_setup_ixs.push(init_bitmap_ext_ix);
    } else {
        bitmap_extension = dlmm::ID;
    }

    for i in 0..position_number {
        let lower_bin_id = min_bin_id + (DEFAULT_BIN_PER_POSITION as i32 * i);
        let upper_bin_id = lower_bin_id + DEFAULT_BIN_PER_POSITION as i32 - 1;
        let upper_bin_id = std::cmp::min(upper_bin_id, max_bin_id - 1);

        let mut instructions = vec![];

        let (position, _bump) =
            derive_position_pda(lb_pair, position_base_kp.pubkey(), lower_bin_id, width);

        let bin_array_account_metas =
            BinArray::get_bin_array_account_metas_coverage(lower_bin_id, upper_bin_id, lb_pair)?;

        let bin_array_indexes =
            BinArray::get_bin_array_indexes_coverage(lower_bin_id, upper_bin_id)?;

        let keys: Vec<_> = [position]
            .into_iter()
            .chain(
                bin_array_indexes
                    .iter()
                    .map(|&index| derive_bin_array_pda(lb_pair, index.into()).0),
            )
            .collect();

        let accounts = rpc_client.get_multiple_accounts(&keys).await?;

        let position_account = accounts.index(0).to_owned();
        if position_account.is_none() {
            let account = dlmm::client::accounts::InitializePositionByOperator {
                position,
                payer: seeder,
                base: position_base_kp.pubkey(),
                lb_pair,
                owner: position_owner,
                operator: seeder,
                operator_token_x: seeder_token_x,
                owner_token_x,
                system_program: solana_sdk::system_program::ID,
                event_authority,
                program: dlmm::ID,
            }
            .to_account_metas(None);

            let ix_data = dlmm::client::args::InitializePositionByOperator {
                lower_bin_id,
                width,
                fee_owner,
                lock_release_point,
            }
            .data();

            let init_position_ix = Instruction {
                program_id: dlmm::ID,
                accounts: account.to_vec(),
                data: ix_data,
            };

            instructions.push(init_position_ix);
        }

        let bin_array_account = &accounts[1..];

        for (account, index) in bin_array_account.iter().zip(bin_array_indexes) {
            if account.is_none() {
                let bin_array = derive_bin_array_pda(lb_pair, index.into()).0;
                let accounts = dlmm::client::accounts::InitializeBinArray {
                    bin_array,
                    lb_pair,
                    funder: seeder,
                    system_program: solana_sdk::system_program::ID,
                }
                .to_account_metas(None);

                let ix_data = dlmm::client::args::InitializeBinArray {
                    index: index.into(),
                }
                .data();

                let init_bin_array_ix = Instruction {
                    program_id: dlmm::ID,
                    accounts,
                    data: ix_data,
                };

                instructions.push(init_bin_array_ix);
            }
        }

        if !instructions.is_empty() {
            if let Some(cu_price_ix) = compute_unit_price.clone() {
                instructions.push(cu_price_ix);
            }

            position_and_bin_array_setup_ixs.push(instructions.clone());
        }

        instructions.clear();

        let position_deposited = position_account
            .map(|account| {
                let state: PositionV2 = bytemuck::pod_read_unaligned(&account.data[8..]);
                state.liquidity_shares.iter().any(|share| *share > 0)
            })
            .unwrap_or(false);

        if !position_deposited {
            let mut bins = vec![];

            for bin_id in lower_bin_id..=upper_bin_id {
                bins.push(CompressedBinDepositAmount {
                    bin_id,
                    amount: *compressed_bin_amount
                        .get(&bin_id)
                        .context("Missing bin amount to deposit")?,
                });
            }

            let ix_data = dlmm::client::args::AddLiquidityOneSidePrecise2 {
                liquidity_parameter: AddLiquiditySingleSidePreciseParameter2 {
                    bins,
                    decompress_multiplier,
                    max_amount: u64::MAX,
                },
                remaining_accounts_info: RemainingAccountsInfo {
                    slices: vec![RemainingAccountsSlice {
                        accounts_type: AccountsType::TransferHookX,
                        length: transfer_hook_x_account.len() as u8,
                    }],
                },
            }
            .data();

            let accounts = dlmm::client::accounts::AddLiquidityOneSidePrecise2 {
                position,
                lb_pair,
                bin_array_bitmap_extension: Some(bitmap_extension),
                user_token: seeder_token_x,
                reserve: lb_pair_state.reserve_x,
                token_mint: lb_pair_state.token_x_mint,
                sender: program.payer(),
                token_program: token_mint_base_owner,
                event_authority,
                program: dlmm::ID,
            }
            .to_account_metas(None);

            let mut accounts = accounts.to_vec();
            accounts.extend_from_slice(&transfer_hook_x_account);
            accounts.extend_from_slice(&bin_array_account_metas);

            let add_liquidity_ix = Instruction {
                program_id: dlmm::ID,
                accounts,
                data: ix_data,
            };

            if instructions.is_empty() {
                if let Some(cu_price_ix) = compute_unit_price.clone() {
                    instructions.push(cu_price_ix);
                }
                instructions.push(ComputeBudgetInstruction::set_compute_unit_limit(800_000));
            }

            instructions.push(add_liquidity_ix);

            // Last position
            if i + 1 == position_number && compression_loss > 0 {
                let loss_includes_transfer_fee = calculate_transfer_fee_included_amount(
                    &token_mint_base_account,
                    compression_loss,
                    clock.epoch,
                )?
                .amount;

                let bin_array_account_metas = BinArray::get_bin_array_account_metas_coverage(
                    upper_bin_id,
                    upper_bin_id,
                    lb_pair,
                )?;

                let ix_data = dlmm::client::args::AddLiquidity2 {
                    liquidity_parameter: LiquidityParameter {
                        amount_x: loss_includes_transfer_fee,
                        amount_y: 0,
                        bin_liquidity_dist: vec![BinLiquidityDistribution {
                            bin_id: upper_bin_id,
                            distribution_x: BASIS_POINT_MAX as u16,
                            distribution_y: BASIS_POINT_MAX as u16,
                        }],
                    },
                    remaining_accounts_info: RemainingAccountsInfo {
                        slices: vec![
                            RemainingAccountsSlice {
                                accounts_type: AccountsType::TransferHookX,
                                length: transfer_hook_x_account.len() as u8,
                            },
                            RemainingAccountsSlice {
                                accounts_type: AccountsType::TransferHookY,
                                length: transfer_hook_y_account.len() as u8,
                            },
                        ],
                    },
                }
                .data();

                let accounts = dlmm::client::accounts::AddLiquidity2 {
                    position,
                    lb_pair,
                    bin_array_bitmap_extension: Some(bitmap_extension),
                    user_token_x: seeder_token_x,
                    user_token_y: seeder_token_y,
                    reserve_x: lb_pair_state.reserve_x,
                    reserve_y: lb_pair_state.reserve_y,
                    token_x_mint: lb_pair_state.token_x_mint,
                    token_y_mint: lb_pair_state.token_y_mint,
                    token_x_program: token_mint_base_owner,
                    token_y_program: token_mint_quote_owner,
                    sender: program.payer(),
                    event_authority,
                    program: dlmm::ID,
                }
                .to_account_metas(None);

                let mut accounts = accounts.to_vec();
                accounts.extend_from_slice(&transfer_hook_x_account);
                accounts.extend_from_slice(&transfer_hook_y_account);
                accounts.extend_from_slice(&bin_array_account_metas);

                let add_liquidity_ix = Instruction {
                    program_id: dlmm::ID,
                    accounts,
                    data: ix_data,
                };

                if instructions.is_empty() {
                    if let Some(cu_price_ix) = compute_unit_price.clone() {
                        instructions.push(cu_price_ix);
                    }

                    instructions.push(ComputeBudgetInstruction::set_compute_unit_limit(800_000));
                }

                instructions.push(add_liquidity_ix);
            }

            if !instructions.is_empty() {
                liquidity_setup_ixs.push(instructions);
            }
        }
    }

    println!("Init token account, bitmap extension and transfer token prove if necessary");
    if !token_account_and_bitmap_ext_and_token_prove_setup_ixs.is_empty() {
        let mut builder = program.request();

        for ix in token_account_and_bitmap_ext_and_token_prove_setup_ixs {
            builder = builder.instruction(ix);
        }

        let signature = builder
            .send_with_spinner_and_config(transaction_config)
            .await;

        println!("{:#?}", signature);
        signature?;
    }
    println!("Init token account, bitmap extension and transfer token prove if necessary - DONE");

    println!("Setup position and bin arrays if necessary");
    if !position_and_bin_array_setup_ixs.is_empty() {
        let mut futures = vec![];

        for ixs in position_and_bin_array_setup_ixs {
            let mut builder = program.request();

            for ix in ixs {
                builder = builder.instruction(ix);
            }

            futures.push(builder.send_with_spinner_and_config(transaction_config));
        }

        let result = try_join_all(futures).await;
        println!("{:#?}", result);
        result?;
    }
    println!("Setup position and bin arrays if necessary - DONE");

    println!("Seed liquidity");
    if !liquidity_setup_ixs.is_empty() {
        let mut futures = vec![];
        for ixs in liquidity_setup_ixs {
            let mut builder = program.request();

            for ix in ixs {
                builder = builder.instruction(ix);
            }

            futures.push(builder.send_with_spinner_and_config(transaction_config));
        }

        let result = try_join_all(futures).await;
        println!("{:#?}", result);
        result?;
    }
    println!("Seed liquidity - DONE");

    Ok(())
}

/// 获取特定bin的存款数量
/// Get deposit amount for a specific bin
fn get_bin_deposit_amount(
    amount: u64,
    bin_step: u16,
    bin_id: i32,
    base_token_decimal: u8,
    quote_token_decimal: u8,
    min_price: f64,
    max_price: f64,
    k: f64,
) -> u64 {
    // 计算下一个bin的累积函数值
    // Calculate cumulative function value for next bin
    let c1 = get_c(
        amount,
        bin_step,
        bin_id + 1,
        base_token_decimal,
        quote_token_decimal,
        min_price,
        max_price,
        k,
    );

    // 计算当前bin的累积函数值
    // Calculate cumulative function value for current bin
    let c0 = get_c(
        amount,
        bin_step,
        bin_id,
        base_token_decimal,
        quote_token_decimal,
        min_price,
        max_price,
        k,
    );

    assert!(c1 > c0);

    // 该bin的存款数量 = c1 - c0
    // Deposit amount for this bin = c1 - c0
    let amount_into_bin = c1 - c0;
    amount_into_bin
}

/// 累积分布函数
/// 公式: c(p) = amount * ((p - min_price)/(max_price - min_price))^k
/// Cumulative distribution function
/// Formula: c(p) = amount * ((p - min_price)/(max_price - min_price))^k
fn get_c(
    amount: u64,
    bin_step: u16,
    bin_id: i32,
    base_token_decimal: u8,
    quote_token_decimal: u8,
    min_price: f64,
    max_price: f64,
    k: f64,
) -> u64 {
    // 计算每lamport价格
    // Calculate price per lamport
    let price_per_lamport = (1.0 + bin_step as f64 / 10000.0).powi(bin_id);

    // 计算当前用户界面价格
    // Calculate current UI price
    let current_price =
        price_per_lamport * 10.0f64.powi(base_token_decimal as i32 - quote_token_decimal as i32);

    // 价格范围和当前价格相对于最小价格的偏移
    // Price range and current price offset from min price
    let price_range = max_price - min_price;
    let current_price_delta_from_min = current_price - min_price;

    // 计算累积分布函数值
    // Calculate cumulative distribution function value
    let c = amount as f64 * ((current_price_delta_from_min / price_range).powf(k));
    c as u64
}

/// 为每个bin生成流动性数量
/// Generate liquidity amounts for each bin
pub fn generate_amount_for_bins(
    bin_step: u16,
    min_bin_id: i32,
    max_bin_id: i32,
    min_price: f64,
    max_price: f64,
    base_token_decimal: u8,
    quote_token_decimal: u8,
    amount: u64,
    k: f64,
) -> Vec<(i32, u64)> {
    let mut total_amount = 0;
    let mut bin_amounts = vec![];

    // 最后一个bin故意不包括，因为对于最后一个bin，c(last_bin +1) - c(last_bin) 会 > 资金数量
    // Last bin is purposely not included because for the last bin, c(last_bin +1) - c(last_bin) will > fund amount
    for bin_id in min_bin_id..max_bin_id {
        // 计算该bin应该存入的数量
        // Calculate the amount to be deposited in this bin
        let bin_amount = get_bin_deposit_amount(
            amount,
            bin_step,
            bin_id,
            base_token_decimal,
            quote_token_decimal,
            min_price,
            max_price,
            k,
        );

        bin_amounts.push((bin_id, bin_amount));
        total_amount += bin_amount;
    }

    // 验证分配给bins的总数量等于资金数量
    // Verify total amount distributed to bins equals funding amount
    assert_eq!(
        total_amount, amount,
        "Amount distributed to bins not equals to funding amount"
    );

    bin_amounts
}
