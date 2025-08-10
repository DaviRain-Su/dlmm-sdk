use std::sync::Arc;

use crate::*;
use anchor_lang::AccountDeserialize;
use anchor_spl::token_interface::Mint;
use commons::dlmm::types::{InitPermissionPairIx, Rounding};

/// 初始化权限流动性池对参数结构体
/// 此结构体包含创建一个需要权限的流动性池对所需的所有参数
/// 权限池对只能由指定的管理员进行管理和配置
#[derive(Debug, Parser)]
pub struct InitPermissionLbPairParameters {
    /// 流动性池对的箱子步长，决定了箱子之间的基点差
    /// 这个值决定了价格在相邻价格箱子之间的最小变动幅度
    pub bin_step: u16,
    /// 流动性池对的代币X铸造地址，例如：BTC，应该是基础代币
    /// 在交易对中，这通常是价值较高或被视为基准的代币
    pub token_mint_x: Pubkey,
    /// 流动性池对的代币Y铸造地址，例如：USDC，应该是报价代币
    /// 在交易对中，这通常是用来定价基础代币的稳定币或参考代币
    pub token_mint_y: Pubkey,
    /// 流动性池对的初始价格，例如：24123.12312412 USDC per 1 BTC
    /// 这个价格将决定活跃箱子的初始位置
    pub initial_price: f64,
    /// 基础密钥对文件路径
    /// 用于生成池对地址的密钥对，必须由管理员控制
    pub base_keypair_path: String,
    /// 基础手续费率（以基点为单位）
    /// 1基点 = 0.01%，用于计算交易手续费
    pub base_fee_bps: u16,
    /// 激活类型
    /// 决定池对的激活方式和权限控制级别
    pub activation_type: u8,
}

/// 执行初始化权限流动性池对操作
/// 
/// 此函数创建一个需要权限控制的流动性池对，只有管理员可以管理此类池对。
/// 权限池对提供了更高级别的控制，包括激活时间、交易权限等。
/// 
/// # 参数
/// * `params` - 初始化权限池对所需的参数
/// * `program` - Solana程序客户端，用于执行链上操作
/// * `transaction_config` - 交易配置，包含确认级别等设置
/// 
/// # 返回值
/// * `Result<Pubkey>` - 成功时返回创建的池对地址，失败时返回错误
/// 
/// # 安全考虑
/// - 只有程序管理员可以调用此函数
/// - 需要提供有效的基础密钥对进行权限验证
/// - 代币铸造地址必须存在且有效
pub async fn execute_initialize_permission_lb_pair<C: Deref<Target = impl Signer> + Clone>(
    params: InitPermissionLbPairParameters,
    program: &Program<C>,
    transaction_config: RpcSendTransactionConfig,
) -> Result<Pubkey> {
    // 解构参数结构体，获取所有必要的配置参数
    let InitPermissionLbPairParameters {
        bin_step,
        token_mint_x,
        token_mint_y,
        initial_price,
        base_keypair_path,
        base_fee_bps,
        activation_type,
    } = params;

    // 读取基础密钥对文件，这个密钥对用于生成池对地址和权限控制
    // 只有拥有此密钥对的管理员才能管理该池对
    let base_keypair =
        Arc::new(read_keypair_file(base_keypair_path).expect("base keypair file not found"));

    let rpc_client = program.rpc();

    // 批量获取代币铸造账户信息，验证代币是否存在且有效
    let mut accounts = rpc_client
        .get_multiple_accounts(&[token_mint_x, token_mint_y])
        .await?;

    // 获取并验证基础代币和报价代币的账户数据
    let token_mint_base_account = accounts[0].take().context("token_mint_base not found")?;
    let token_mint_quote_account = accounts[1].take().context("token_mint_quote not found")?;

    // 反序列化代币铸造账户数据，获取代币的精度等重要信息
    let token_mint_base = Mint::try_deserialize(&mut token_mint_base_account.data.as_ref())?;
    let token_mint_quote = Mint::try_deserialize(&mut token_mint_quote_account.data.as_ref())?;

    // 将初始价格转换为以lamport为单位的价格
    // 考虑两个代币的小数位数差异，确保价格计算的准确性
    let price_per_lamport = price_per_token_to_per_lamport(
        initial_price,
        token_mint_base.decimals,
        token_mint_quote.decimals,
    )
    .context("price_per_token_to_per_lamport overflow")?;

    // 根据初始价格计算活跃箱子ID
    // 这决定了流动性池对开始交易时的价格点
    let computed_active_id = get_id_from_price(bin_step, &price_per_lamport, Rounding::Up)
        .context("get_id_from_price overflow")?;

    // 生成权限流动性池对的程序衍生地址(PDA)
    // 使用基础密钥对、代币铸造地址和箱子步长作为种子
    let (lb_pair, _bump) =
        derive_permission_lb_pair_pda(base_keypair.pubkey(), token_mint_x, token_mint_y, bin_step);

    // 检查池对是否已经存在，如果存在则直接返回其地址
    if program.rpc().get_account_data(&lb_pair).await.is_ok() {
        return Ok(lb_pair);
    }

    // 生成代币储备金库的PDA，用于存放池对中的代币
    let (reserve_x, _bump) = derive_reserve_pda(token_mint_x, lb_pair);
    let (reserve_y, _bump) = derive_reserve_pda(token_mint_y, lb_pair);
    
    // 生成预言机账户PDA，用于记录和提供价格信息
    let (oracle, _bump) = derive_oracle_pda(lb_pair);

    // 生成事件权限账户PDA，用于记录和发出事件日志
    let (event_authority, _bump) = derive_event_authority_pda();

    // 生成代币徽章PDA，用于代币的额外权限和标识管理
    let (token_badge_x, _bump) = derive_token_badge_pda(token_mint_x);
    let (token_badge_y, _bump) = derive_token_badge_pda(token_mint_y);

    // 检查代币徽章账户是否存在
    let accounts = rpc_client
        .get_multiple_accounts(&[token_badge_x, token_badge_y])
        .await?;

    // 如果代币徽章存在则使用其地址，否则使用程序ID作为默认值
    // 这允许支持带有特殊权限标识的代币和普通代币
    let token_badge_x = accounts[0]
        .as_ref()
        .map(|_| token_badge_x)
        .or(Some(dlmm::ID));

    let token_badge_y = accounts[1]
        .as_ref()
        .map(|_| token_badge_y)
        .or(Some(dlmm::ID));

    // 构建初始化权限流动性池对所需的账户列表
    // 这些账户将在链上指令执行时被使用和验证
    let accounts = dlmm::client::accounts::InitializePermissionLbPair {
        lb_pair,                                                    // 流动性池对账户
        bin_array_bitmap_extension: Some(dlmm::ID),                // 箱子数组位图扩展
        reserve_x,                                                  // 代币X储备金库
        reserve_y,                                                  // 代币Y储备金库
        token_mint_x,                                               // 代币X铸造地址
        token_mint_y,                                               // 代币Y铸造地址
        token_badge_x,                                              // 代币X徽章（如果存在）
        token_badge_y,                                              // 代币Y徽章（如果存在）
        token_program_x: token_mint_base_account.owner,             // 代币X的程序ID
        token_program_y: token_mint_quote_account.owner,            // 代币Y的程序ID
        oracle,                                                     // 预言机账户
        admin: program.payer(),                                     // 管理员账户（交易付款人）
        rent: solana_sdk::sysvar::rent::ID,                        // 租金系统变量
        system_program: solana_sdk::system_program::ID,            // 系统程序
        event_authority,                                            // 事件权限账户
        program: dlmm::ID,                                          // DLMM程序ID
        base: base_keypair.pubkey(),                                // 基础密钥对公钥
    }
    .to_account_metas(None);

    // 根据基础手续费基点计算基础因子和手续费幂因子
    // 这些参数用于动态计算交易手续费
    let (base_factor, base_fee_power_factor) =
        compute_base_factor_from_fee_bps(bin_step, base_fee_bps)?;

    // 构建初始化指令的数据负载
    let data = dlmm::client::args::InitializePermissionLbPair {
        ix_data: InitPermissionPairIx {
            active_id: computed_active_id,                          // 计算得出的活跃箱子ID
            bin_step,                                               // 箱子步长
            base_factor,                                            // 基础因子
            activation_type,                                        // 激活类型
            base_fee_power_factor,                                  // 基础手续费幂因子
            protocol_share: ILM_PROTOCOL_SHARE,                     // 协议分成比例
        },
    }
    .data();

    // 构建完整的初始化权限流动性池对指令
    let init_pair_ix = Instruction {
        program_id: dlmm::ID,
        accounts,
        data,
    };

    // 构建并发送交易请求
    let request_builder = program.request();
    let signature = request_builder
        .instruction(init_pair_ix)                                  // 添加初始化指令
        .signer(base_keypair)                                       // 添加基础密钥对签名
        .send_with_spinner_and_config(transaction_config)          // 发送交易并等待确认
        .await;

    println!("Initialize Permission LB pair {lb_pair}. Signature: {signature:#?}");

    // 检查交易是否成功执行
    signature?;

    // 输出创建的池对地址供后续使用
    println!("{lb_pair}");

    // 返回成功创建的权限流动性池对地址
    Ok(lb_pair)
}
