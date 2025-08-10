use crate::*;
use instructions::*;

/// 领取奖励的参数结构体
/// Parameters for claiming rewards
#[derive(Debug, Parser)]
pub struct ClaimRewardParams {
    /// 流动性交易对地址
    /// Liquidity pair address
    pub lb_pair: Pubkey,
    /// 奖励索引（0或1）
    /// Reward index (0 or 1)
    pub reward_index: u64,
    /// 仓位地址
    /// Position address
    pub position: Pubkey,
}

/// 执行领取奖励指令
/// Executes the claim reward instruction
/// 
/// # 参数 / Parameters
/// * `params` - 领取奖励的参数 / Parameters for reward claiming
/// * `program` - Solana程序引用 / Solana program reference
/// * `transaction_config` - 交易配置 / Transaction configuration
/// * `compute_unit_price` - 计算单元价格指令（可选）/ Compute unit price instruction (optional)
/// 
/// # 功能说明 / Functionality
/// 从指定的流动性仓位中领取累积的奖励代币到用户的代币账户
/// Claims accumulated reward tokens from the specified liquidity position to user's token account
pub async fn execute_claim_reward<C: Deref<Target = impl Signer> + Clone>(
    params: ClaimRewardParams,
    program: &Program<C>,
    transaction_config: RpcSendTransactionConfig,
    compute_unit_price: Option<Instruction>,
) -> Result<()> {
    let ClaimRewardParams {
        lb_pair,
        reward_index,
        position,
    } = params;

    let rpc_client = program.rpc();
    
    // 派生奖励金库PDA
    // Derive reward vault PDA
    let (reward_vault, _bump) = derive_reward_vault_pda(lb_pair, reward_index);

    // 获取流动性交易对状态数据
    // Get liquidity pair state data
    let lb_pair_state: LbPair = rpc_client
        .get_account_and_deserialize(&lb_pair, |account| {
            Ok(bytemuck::pod_read_unaligned(&account.data[8..]))
        })
        .await?;

    // 获取仓位状态数据
    // Get position state data
    let position_state: PositionV2 = rpc_client
        .get_account_and_deserialize(&position, |account| {
            Ok(bytemuck::pod_read_unaligned(&account.data[8..]))
        })
        .await?;

    // 获取指定索引的奖励信息
    // Get reward information for specified index
    let reward_info = lb_pair_state.reward_infos[reward_index as usize];
    let reward_mint = reward_info.mint;

    // 获取奖励代币的程序所有者
    // Get reward token's program owner
    let reward_mint_program = rpc_client.get_account(&reward_mint).await?.owner;

    // 创建或获取用户的奖励代币账户
    // Create or get user's reward token account
    let user_token_account = get_or_create_ata(
        program,
        transaction_config,
        reward_mint,
        program.payer(),
        compute_unit_price.clone(),
    )
    .await?;

    // 派生事件权限PDA
    // Derive event authority PDA
    let (event_authority, _bump) = derive_event_authority_pda();

    // 构建主要账户信息
    // Build main account information
    let main_accounts = dlmm::client::accounts::ClaimReward2 {
        lb_pair,                                // 流动性交易对 / Liquidity pair
        reward_vault,                           // 奖励金库 / Reward vault
        reward_mint,                            // 奖励代币铸币账户 / Reward token mint
        memo_program: spl_memo::ID,             // 备忘录程序 / Memo program
        token_program: reward_mint_program,     // 代币程序 / Token program
        position,                               // 仓位账户 / Position account
        user_token_account,                     // 用户代币账户 / User token account
        sender: program.payer(),                // 发送者 / Sender
        event_authority,                        // 事件权限 / Event authority
        program: dlmm::ID,                      // DLMM程序ID / DLMM program ID
    }
    .to_account_metas(None);

    // 初始化剩余账户信息和Token 2022相关账户
    // Initialize remaining accounts info and Token 2022 related accounts
    let mut remaining_accounts_info = RemainingAccountsInfo { slices: vec![] };
    let mut token_2022_remaining_accounts = vec![];

    // 获取可能的Token 2022相关指令数据和账户（针对奖励操作）
    // Get potential Token 2022 related instruction data and accounts (for reward action)
    if let Some((slices, transfer_hook_remaining_accounts)) =
        get_potential_token_2022_related_ix_data_and_accounts(
            &lb_pair_state,
            program.rpc(),
            ActionType::Reward(reward_index as usize),
        )
        .await?
    {
        remaining_accounts_info.slices = slices;
        token_2022_remaining_accounts.extend(transfer_hook_remaining_accounts);
    };

    // 分块处理仓位的bin范围以领取奖励
    // Process position bin range in chunks to claim rewards
    for (min_bin_id, max_bin_id) in
        position_bin_range_chunks(position_state.lower_bin_id, position_state.upper_bin_id)
    {
        // 构建领取奖励指令数据
        // Build claim reward instruction data
        let data = dlmm::client::args::ClaimReward2 {
            reward_index,   // 奖励索引 / Reward index
            min_bin_id,     // 最小bin ID / Minimum bin ID
            max_bin_id,     // 最大bin ID / Maximum bin ID
            remaining_accounts_info: remaining_accounts_info.clone(),
        }
        .data();

        // 获取当前块覆盖的bin数组账户元数据
        // Get bin array account metadata covered by current chunk
        let bin_arrays_account_meta =
            position_state.get_bin_array_accounts_meta_coverage_by_chunk(min_bin_id, max_bin_id)?;

        // 组合所有必需的账户
        // Combine all required accounts
        let accounts = [
            main_accounts.to_vec(),
            token_2022_remaining_accounts.clone(),
            bin_arrays_account_meta,
        ]
        .concat();

        // 创建领取奖励指令
        // Create claim reward instruction
        let claim_reward_ix = Instruction {
            program_id: dlmm::ID,
            accounts,
            data,
        };

        // 构建并发送交易
        // Build and send transaction
        let request_builder = program.request();
        let signature = request_builder
            .instruction(claim_reward_ix)
            .send_with_spinner_and_config(transaction_config)
            .await;

        println!("Claim reward. Signature: {:#?}", signature);

        signature?;
    }

    Ok(())
}
