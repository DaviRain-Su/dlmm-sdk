use crate::*;
use anchor_lang::AccountDeserialize;
use anchor_spl::token_interface::Mint;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;
use solana_client::{
    rpc_config::{RpcAccountInfoConfig, RpcProgramAccountsConfig},
    rpc_filter::{Memcmp, RpcFilterType},
};

/// 将手续费率转换为手续费百分比
/// Converts fee rate to fee percentage
fn fee_rate_to_fee_pct(fee_rate: u128) -> Option<Decimal> {
    let fee_rate = Decimal::from_u128(fee_rate)?.checked_div(Decimal::from(FEE_PRECISION))?;
    fee_rate.checked_mul(Decimal::ONE_HUNDRED)
}

/// 显示交易对信息的参数结构体
/// Parameters for showing pair information
#[derive(Debug, Parser)]
pub struct ShowPairParams {
    /// 流动性交易对地址
    /// Liquidity pair address
    pub lb_pair: Pubkey,
}

/// 执行显示交易对信息指令
/// Executes the show pair information instruction
/// 
/// # 参数 / Parameters
/// * `params` - 显示交易对信息的参数 / Parameters for showing pair information
/// * `program` - Solana程序引用 / Solana program reference
/// 
/// # 功能说明 / Functionality
/// 显示指定流动性交易对的详细信息，包括价格、手续费率和bin流动性分布
/// Shows detailed information of the specified liquidity pair, including price, fee rates, and bin liquidity distribution
pub async fn execute_show_pair<C: Deref<Target = impl Signer> + Clone>(
    params: ShowPairParams,
    program: &Program<C>,
) -> Result<()> {
    let ShowPairParams { lb_pair } = params;
    let rpc_client = program.rpc();

    // 获取流动性交易对状态数据
    // Get liquidity pair state data
    let lb_pair_state: LbPair = rpc_client
        .get_account_and_deserialize(&lb_pair, |account| {
            Ok(bytemuck::pod_read_unaligned(&account.data[8..]))
        })
        .await?;

    // 设置过滤器以获取与此交易对相关的所有bin数组
    // Set up filter to get all bin arrays related to this pair
    let lb_pair_filter = RpcFilterType::Memcmp(Memcmp::new_base58_encoded(16, &lb_pair.to_bytes()));
    let account_config = RpcAccountInfoConfig {
        encoding: Some(UiAccountEncoding::Base64),
        ..Default::default()
    };
    let config = RpcProgramAccountsConfig {
        filters: Some(vec![lb_pair_filter]),
        account_config,
        ..Default::default()
    };

    // 获取所有相关的bin数组账户
    // Get all related bin array accounts
    let mut bin_arrays: Vec<(Pubkey, BinArray)> = rpc_client
        .get_program_accounts_with_config(&dlmm::ID, config)
        .await?
        .into_iter()
        .filter_map(|(key, account)| {
            let bin_array = bytemuck::pod_read_unaligned(&account.data[8..]);
            Some((key, bin_array))
        })
        .collect();

    // 按bin数组索引排序
    // Sort by bin array index
    bin_arrays.sort_by(|a, b| a.1.index.cmp(&b.1.index));

    // 打印交易对状态信息
    // Print pair state information
    println!("{:#?}", lb_pair_state);

    // 遍历所有bin数组并显示有流动性的bin
    // Iterate through all bin arrays and show bins with liquidity
    for (_, bin_array) in bin_arrays {
        // 获取当前bin数组的起始bin ID
        // Get the starting bin ID of current bin array
        let (mut lower_bin_id, _) =
            BinArray::get_bin_array_lower_upper_bin_id(bin_array.index as i32)?;
        
        // 遍历bin数组中的每个bin
        // Iterate through each bin in the bin array
        for bin in bin_array.bins.iter() {
            let total_amount = bin.amount_x + bin.amount_y;
            // 只显示有流动性的bin
            // Only show bins with liquidity
            if total_amount > 0 {
                println!(
                    "Bin: {}, X: {}, Y: {}",
                    lower_bin_id, bin.amount_x, bin.amount_y
                );
            }
            lower_bin_id += 1;
        }
    }

    // 获取X和Y代币的铸币账户信息
    // Get X and Y token mint account information
    let mut accounts = rpc_client
        .get_multiple_accounts(&[lb_pair_state.token_x_mint, lb_pair_state.token_y_mint])
        .await?;

    let token_x_account = accounts[0].take().context("token_mint_base not found")?;
    let token_y_account = accounts[1].take().context("token_mint_quote not found")?;

    // 反序列化代币铸币数据
    // Deserialize token mint data
    let x_mint = Mint::try_deserialize(&mut token_x_account.data.as_ref())?;
    let y_mint = Mint::try_deserialize(&mut token_y_account.data.as_ref())?;

    // 从当前活跃bin ID获取Q64x64格式的价格
    // Get Q64x64 format price from current active bin ID
    let q64x64_price = get_price_from_id(lb_pair_state.active_id, lb_pair_state.bin_step)?;
    
    // 将Q64x64价格转换为十进制价格（每lamport）
    // Convert Q64x64 price to decimal price (per lamport)
    let decimal_price_per_lamport =
        q64x64_price_to_decimal(q64x64_price).context("q64x64 price to decimal overflow")?;

    // 将每lamport的价格转换为每代币的价格（考虑小数位数）
    // Convert per-lamport price to per-token price (considering decimals)
    let token_price = price_per_lamport_to_price_per_token(
        decimal_price_per_lamport
            .to_f64()
            .context("Decimal conversion to f64 fail")?,
        x_mint.decimals,
        y_mint.decimals,
    )
    .context("price_per_lamport_to_price_per_token overflow")?;

    // 计算各种手续费率
    // Calculate various fee rates
    let base_fee_rate = fee_rate_to_fee_pct(lb_pair_state.get_total_fee()?)
        .context("get_total_fee convert to percentage overflow")?;
    let variable_fee_rate = fee_rate_to_fee_pct(lb_pair_state.get_variable_fee()?)
        .context("get_total_fee convert to percentage overflow")?;
    let current_fee_rate = fee_rate_to_fee_pct(lb_pair_state.get_total_fee()?)
        .context("get_total_fee convert to percentage overflow")?;

    // 显示价格和手续费信息
    // Display price and fee information
    println!("Current price {}", token_price);         // 当前价格
    println!("Base fee rate {}%", base_fee_rate);      // 基础手续费率
    println!("Volatile fee rate {}%", variable_fee_rate); // 波动手续费率
    println!("Current fee rate {}%", current_fee_rate); // 当前总手续费率

    Ok(())
}
