use std::sync::Arc;

use crate::*;

/// 初始化仓位的参数结构体
/// Parameters for initializing a position
#[derive(Debug, Parser)]
pub struct InitPositionParams {
    /// 流动性交易对的地址
    /// Address of the liquidity pair.
    pub lb_pair: Pubkey,
    /// bin范围的下界ID
    /// Lower bound of the bin range.
    #[clap(long, allow_negative_numbers = true)]
    pub lower_bin_id: i32,
    /// 仓位的宽度，从1到70
    /// Width of the position. Start with 1 until 70.
    pub width: i32,
}

/// 执行初始化仓位指令
/// Executes the initialize position instruction
/// 
/// # 参数 / Parameters
/// * `params` - 初始化仓位的参数 / Parameters for position initialization
/// * `program` - Solana程序引用 / Solana program reference
/// * `transaction_config` - 交易配置 / Transaction configuration
/// 
/// # 返回值 / Returns
/// 返回新创建的仓位公钥 / Returns the public key of the newly created position
/// 
/// # 功能说明 / Functionality
/// 在指定的流动性交易对上创建一个新的流动性仓位，设置bin ID范围和宽度
/// Creates a new liquidity position on the specified liquidity pair with bin ID range and width
pub async fn execute_initialize_position<C: Deref<Target = impl Signer> + Clone>(
    params: InitPositionParams,
    program: &Program<C>,
    transaction_config: RpcSendTransactionConfig,
) -> Result<Pubkey> {
    let InitPositionParams {
        lb_pair,
        lower_bin_id,
        width,
    } = params;

    // 创建新的仓位密钥对
    // Create a new position keypair
    let position_keypair = Arc::new(Keypair::new());

    // 派生事件权限PDA
    // Derive event authority PDA
    let (event_authority, _bump) = derive_event_authority_pda();

    // 构建初始化仓位所需的账户
    // Build accounts required for position initialization
    let accounts = dlmm::client::accounts::InitializePosition {
        lb_pair,                                     // 流动性交易对账户 / Liquidity pair account
        payer: program.payer(),                      // 支付者账户 / Payer account
        position: position_keypair.pubkey(),         // 新仓位账户 / New position account
        owner: program.payer(),                      // 仓位所有者 / Position owner
        rent: solana_sdk::sysvar::rent::ID,          // Rent系统变量 / Rent sysvar
        system_program: solana_sdk::system_program::ID, // 系统程序 / System program
        event_authority,                             // 事件权限 / Event authority
        program: dlmm::ID,                           // DLMM程序ID / DLMM program ID
    }
    .to_account_metas(None);

    // 构建指令数据
    // Build instruction data
    let data = dlmm::client::args::InitializePosition {
        lower_bin_id,  // 下界bin ID / Lower bin ID
        width,         // 仓位宽度 / Position width
    }
    .data();

    // 创建初始化仓位指令
    // Create initialize position instruction
    let init_position_ix = Instruction {
        program_id: dlmm::ID,
        data,
        accounts,
    };

    // 构建并发送交易
    // Build and send transaction
    let request_builder = program.request();
    let signature = request_builder
        .instruction(init_position_ix)
        .signer(position_keypair.clone())  // 仓位密钥对需要签名 / Position keypair needs to sign
        .send_with_spinner_and_config(transaction_config)
        .await;

    println!(
        "Initialize position {}. Signature: {signature:#?}",
        position_keypair.pubkey()
    );

    signature?;

    Ok(position_keypair.pubkey())
}
