use anchor_lang::Discriminator;

use crate::*;

/// 显示仓位信息的参数结构体
/// Parameters for showing position information
#[derive(Debug, Parser)]
pub struct ShowPositionParams {
    /// 仓位地址
    /// Position address
    pub position: Pubkey,
}

/// 执行显示仓位信息指令
/// Executes the show position information instruction
/// 
/// # 参数 / Parameters
/// * `params` - 显示仓位信息的参数 / Parameters for showing position information
/// * `program` - Solana程序引用 / Solana program reference
/// 
/// # 功能说明 / Functionality
/// 显示指定仓位的详细信息，包括仓位状态、流动性分布和所有者信息
/// Shows detailed information of the specified position, including position state, liquidity distribution, and owner information
pub async fn execute_show_position<C: Deref<Target = impl Signer> + Clone>(
    params: ShowPositionParams,
    program: &Program<C>,
) -> Result<()> {
    let ShowPositionParams { position } = params;

    let rpc_client = program.rpc();
    
    // 获取仓位账户数据
    // Get position account data
    let position_account = rpc_client.get_account(&position).await?;

    // 读取账户鉴别器以确定仓位版本
    // Read account discriminator to determine position version
    let mut disc = [0u8; 8];
    disc.copy_from_slice(&position_account.data[..8]);

    // 根据鉴别器确定是旧版本还是新版本的仓位账户
    // Determine if it's old version or new version position account based on discriminator
    if disc == Position::DISCRIMINATOR {
        // 旧版本仓位（Position）
        // Old version position (Position)
        let position_state: Position = bytemuck::pod_read_unaligned(&position_account.data[8..]);
        println!("{:#?}", position_state);
    } else if disc == PositionV2::DISCRIMINATOR {
        // 新版本仓位（PositionV2）
        // New version position (PositionV2)
        let position_state: PositionV2 = bytemuck::pod_read_unaligned(&position_account.data[8..]);
        println!("{:#?}", position_state);
    } else {
        // 无效的仓位账户
        // Invalid position account
        bail!("Not a valid position account");
    };

    Ok(())
}
