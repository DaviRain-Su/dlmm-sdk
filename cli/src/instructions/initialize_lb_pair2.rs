use crate::*;
use anchor_lang::AccountDeserialize;
use anchor_spl::token_interface::Mint;

/// 初始化流动性对参数（版本2）
#[derive(Debug, Parser)]
pub struct InitLbPair2Params {
    /// Preset parameter pubkey. Get the pubkey from list_all_binstep command.
    /// 预设参数公钥。通过list_all_binstep命令获取
    pub preset_parameter: Pubkey,
    /// Token X mint of the liquidity pair. Eg: BTC. This should be the base token.
    /// 流动性对的X代币铸造地址。例如：BTC。这应该是基础代币
    pub token_mint_x: Pubkey,
    /// Token Y mint of the liquidity pair. Eg: USDC. This should be the quote token.
    /// 流动性对的Y代币铸造地址。例如：USDC。这应该是报价代币
    pub token_mint_y: Pubkey,
    /// The initial price of the liquidity pair. Eg: 24123.12312412 USDC per 1 BTC.
    /// 流动性对的初始价格。例如：每1个BTC价值24123.12312412 USDC
    pub initial_price: f64,
}

/// 执行初始化流动性对（版本2）
/// 
/// # 参数
/// * `params` - 初始化参数
/// * `program` - Anchor程序客户端
/// * `transaction_config` - 交易配置
/// 
/// # 返回
/// * 创建的流动性对地址
/// 
/// # 功能
/// 1. 验证代币铸造账户
/// 2. 计算初始活跃bin ID
/// 3. 创建必要的PDA账户
/// 4. 发送初始化交易
pub async fn execute_initialize_lb_pair2<C: Deref<Target = impl Signer> + Clone>(
    params: InitLbPair2Params,
    program: &Program<C>,
    transaction_config: RpcSendTransactionConfig,
) -> Result<Pubkey> {
    let InitLbPair2Params {
        preset_parameter,
        token_mint_x,
        token_mint_y,
        initial_price,
    } = params;

    let rpc_client = program.rpc();

    // 批量获取代币铸造账户信息
    let mut accounts = rpc_client
        .get_multiple_accounts(&[token_mint_x, token_mint_y])
        .await?;

    let token_mint_base_account = accounts[0].take().context("token_mint_base not found")?;
    let token_mint_quote_account = accounts[1].take().context("token_mint_quote not found")?;

    // 反序列化代币铸造信息，获取小数位数
    let token_mint_base = Mint::try_deserialize(&mut token_mint_base_account.data.as_ref())?;
    let token_mint_quote = Mint::try_deserialize(&mut token_mint_quote_account.data.as_ref())?;

    // 将UI价格转换为每lamport价格
    let price_per_lamport = price_per_token_to_per_lamport(
        initial_price,
        token_mint_base.decimals,
        token_mint_quote.decimals,
    )
    .context("price_per_token_to_per_lamport overflow")?;

    // 获取预设参数状态
    let preset_parameter_state: PresetParameter2 = rpc_client
        .get_account_and_deserialize(&preset_parameter, |account| {
            Ok(bytemuck::pod_read_unaligned(&account.data[8..]))
        })
        .await?;

    // 获取bin步长，用于计算活跃bin ID
    let bin_step = preset_parameter_state.bin_step;

    // 根据初始价格计算活跃bin ID（向上舍入）
    let computed_active_id = get_id_from_price(bin_step, &price_per_lamport, Rounding::Up)
        .context("get_id_from_price overflow")?;

    // 派生流动性对PDA地址
    let (lb_pair, _bump) =
        derive_lb_pair_with_preset_parameter_key(preset_parameter, token_mint_x, token_mint_y);

    // 如果流动性对已存在，直接返回地址
    if program.rpc().get_account_data(&lb_pair).await.is_ok() {
        return Ok(lb_pair);
    }

    let (reserve_x, _bump) = derive_reserve_pda(token_mint_x, lb_pair);
    let (reserve_y, _bump) = derive_reserve_pda(token_mint_y, lb_pair);
    let (oracle, _bump) = derive_oracle_pda(lb_pair);

    let (event_authority, _bump) = derive_event_authority_pda();
    let (token_badge_x, _bump) = derive_token_badge_pda(token_mint_x);
    let (token_badge_y, _bump) = derive_token_badge_pda(token_mint_y);

    let accounts = rpc_client
        .get_multiple_accounts(&[token_badge_x, token_badge_y])
        .await?;

    let token_badge_x = accounts[0]
        .as_ref()
        .map(|_| token_badge_x)
        .or(Some(dlmm::ID));

    let token_badge_y = accounts[1]
        .as_ref()
        .map(|_| token_badge_y)
        .or(Some(dlmm::ID));

    let accounts = dlmm::client::accounts::InitializeLbPair2 {
        lb_pair,
        bin_array_bitmap_extension: Some(dlmm::ID),
        reserve_x,
        reserve_y,
        token_mint_x,
        token_mint_y,
        oracle,
        funder: program.payer(),
        token_badge_x,
        token_badge_y,
        token_program_x: token_mint_base_account.owner,
        token_program_y: token_mint_quote_account.owner,
        preset_parameter,
        system_program: solana_sdk::system_program::ID,
        event_authority,
        program: dlmm::ID,
    }
    .to_account_metas(None);

    let data = dlmm::client::args::InitializeLbPair2 {
        params: InitializeLbPair2Params {
            active_id: computed_active_id,
            padding: [0u8; 96],
        },
    }
    .data();

    let init_pair_ix = Instruction {
        program_id: dlmm::ID,
        data,
        accounts,
    };

    let request_builder = program.request();

    let signature = request_builder
        .instruction(init_pair_ix)
        .send_with_spinner_and_config(transaction_config)
        .await;

    println!("Initialize LB pair2 {lb_pair}. Signature: {signature:#?}");

    signature?;

    Ok(lb_pair)
}
