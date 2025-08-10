use crate::*;
use solana_client::rpc_config::{RpcAccountInfoConfig, RpcProgramAccountsConfig};

/// 获取所有头寸的参数结构体
/// Parameters for getting all positions
#[derive(Debug, Parser)]
pub struct GetAllPositionsParams {
    /// 流动性对的地址 / Address of the liquidity pair
    #[clap(long)]
    lb_pair: Pubkey,
    /// 头寸所有者 / Owner of position
    #[clap(long)]
    owner: Pubkey,
}

/// 执行获取所有头寸操作
/// Execute get all positions operation
pub async fn execute_get_all_positions<C: Deref<Target = impl Signer> + Clone>(
    program: &Program<C>,
    params: GetAllPositionsParams,
) -> Result<()> {
    // 解构参数
    // Destructure parameters
    let GetAllPositionsParams { lb_pair, owner } = params;

    let rpc_client = program.rpc();

    // 设置账户配置，使用Base64编码
    // Set account configuration with Base64 encoding
    let account_config = RpcAccountInfoConfig {
        encoding: Some(UiAccountEncoding::Base64),
        ..Default::default()
    };
    
    // 设置程序账户查询配置，按钱包和流动性对过滤头寸
    // Set program account query configuration, filter positions by wallet and pair
    let config = RpcProgramAccountsConfig {
        filters: Some(position_filter_by_wallet_and_pair(owner, lb_pair)),
        account_config,
        ..Default::default()
    };

    // 获取所有匹配的头寸账户
    // Get all matching position accounts
    let accounts = rpc_client
        .get_program_accounts_with_config(&dlmm::ID, config)
        .await?;

    // 遍历并显示所有头寸信息
    // Iterate and display all position information
    for (position_key, position_raw_account) in accounts {
        // 解析头寸状态
        // Parse position state
        let position_state: PositionV2 =
            bytemuck::pod_read_unaligned(&position_raw_account.data[8..]);
        println!(
            "Position {} fee owner {}",
            position_key, position_state.fee_owner
        );
    }

    Ok(())
}
