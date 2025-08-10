use crate::*;

/// 初始化bin数组的参数结构体
/// Parameters for initializing bin array
#[derive(Debug, Parser)]
pub struct InitBinArrayParams {
    /// bin数组的索引
    /// Index of the bin array.
    #[clap(long, allow_negative_numbers = true)]
    pub bin_array_index: i64,
    /// 流动性交易对的地址
    /// Address of the liquidity pair.
    pub lb_pair: Pubkey,
}

/// 执行初始化bin数组指令
/// Executes the initialize bin array instruction
/// 
/// # 参数 / Parameters
/// * `params` - 初始化bin数组的参数 / Parameters for bin array initialization
/// * `program` - Solana程序引用 / Solana program reference
/// * `transaction_config` - 交易配置 / Transaction configuration
/// 
/// # 返回值 / Returns
/// 返回新创建的bin数组公钥 / Returns the public key of the newly created bin array
/// 
/// # 功能说明 / Functionality
/// 为指定的流动性交易对创建一个新的bin数组，用于存储流动性分布
/// Creates a new bin array for the specified liquidity pair to store liquidity distribution
pub async fn execute_initialize_bin_array<C: Deref<Target = impl Signer> + Clone>(
    params: InitBinArrayParams,
    program: &Program<C>,
    transaction_config: RpcSendTransactionConfig,
) -> Result<Pubkey> {
    let InitBinArrayParams {
        lb_pair,
        bin_array_index,
    } = params;

    // 派生bin数组的PDA地址
    // Derive bin array PDA address
    let (bin_array, _bump) = derive_bin_array_pda(lb_pair, bin_array_index);

    // 构建初始化bin数组所需的账户
    // Build accounts required for bin array initialization
    let accounts = dlmm::client::accounts::InitializeBinArray {
        bin_array,                              // bin数组账户 / Bin array account
        funder: program.payer(),                // 资金提供者 / Funder
        lb_pair,                                // 流动性交易对 / Liquidity pair
        system_program: solana_sdk::system_program::ID, // 系统程序 / System program
    }
    .to_account_metas(None);

    // 构建指令数据
    // Build instruction data
    let data = dlmm::client::args::InitializeBinArray {
        index: bin_array_index,  // bin数组索引 / Bin array index
    }
    .data();

    // 创建初始化bin数组指令
    // Create initialize bin array instruction
    let init_bin_array_ix = Instruction {
        program_id: dlmm::ID,
        accounts,
        data,
    };

    // 构建并发送交易
    // Build and send transaction
    let request_builder = program.request();
    let signature = request_builder
        .instruction(init_bin_array_ix)
        .send_with_spinner_and_config(transaction_config)
        .await;

    println!("Initialize Bin Array {bin_array}. Signature: {signature:#?}");

    signature?;

    Ok(bin_array)
}
