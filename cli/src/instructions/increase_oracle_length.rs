use crate::*;
use anchor_client::solana_sdk;

/// 增加预言机长度的参数结构体
/// Parameters for increasing oracle length
#[derive(Debug, Parser)]
pub struct IncreaseOracleLengthParams {
    /// 流动性对地址 / Liquidity pair address
    pub lb_pair: Pubkey,
    /// 要增加的长度 / Length to add
    pub length_to_add: u64,
}

/// 执行增加预言机长度操作
/// Execute increase oracle length operation
pub async fn execute_increase_oracle_length<C: Deref<Target = impl Signer> + Clone>(
    params: IncreaseOracleLengthParams,
    program: &Program<C>,
    transaction_config: RpcSendTransactionConfig,
) -> Result<()> {
    // 解构参数
    // Destructure parameters
    let IncreaseOracleLengthParams {
        lb_pair,
        length_to_add,
    } = params;

    // 派生预言机PDA和事件权限PDA
    // Derive oracle PDA and event authority PDA
    let (oracle, _) = derive_oracle_pda(lb_pair);
    let (event_authority, _bump) = derive_event_authority_pda();

    // 准备账户元数据
    // Prepare account metadata
    let accounts = dlmm::client::accounts::IncreaseOracleLength {
        funder: program.payer(),
        oracle,
        system_program: solana_sdk::system_program::ID,
        event_authority,
        program: dlmm::ID,
    }
    .to_account_metas(None);

    // 准备指令数据
    // Prepare instruction data
    let data = dlmm::client::args::IncreaseOracleLength { length_to_add }.data();

    // 创建增加预言机长度指令
    // Create increase oracle length instruction
    let increase_length_ix = Instruction {
        program_id: dlmm::ID,
        accounts,
        data,
    };

    // 构建并发送交易
    // Build and send transaction
    let request_builder = program.request();
    let signature = request_builder
        .instruction(increase_length_ix)
        .send_with_spinner_and_config(transaction_config)
        .await;

    println!("Increase oracle {oracle} length. Signature: {signature:#?}");

    signature?;

    Ok(())
}
