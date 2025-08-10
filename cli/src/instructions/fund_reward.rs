use crate::*;
use instructions::*;

/// 资助奖励系统的参数结构体
/// 该功能允许授权的资助者为奖励系统添加资金
/// 资金将按照设定的时间周期分发给流动性提供者
#[derive(Debug, Parser)]
pub struct FundRewardParams {
    /// 流动性池对的地址
    /// 需要资助奖励的池对
    pub lb_pair: Pubkey,
    /// 奖励索引
    /// 指定要资助的奖励系统索引
    pub reward_index: u64,
    /// 资助金额
    /// 添加到奖励池中的代币数量
    pub funding_amount: u64,
}

/// 执行资助奖励系统操作
/// 
/// 此函数允许授权的资助者为指定的奖励系统添加资金。
/// 资助是奖励系统正常运作的关键，用于：
/// - 维持奖励的持续分发
/// - 增加奖励吸引力和竞争力
/// - 激励更多流动性提供者参与
/// - 维持池对的长期活跃度
/// 
/// # 参数
/// * `params` - 资助参数，包括池对、奖励索引和资助金额
/// * `program` - Solana程序客户端，用于执行链上操作
/// * `transaction_config` - 交易配置，包含确认级别等设置
/// * `compute_unit_price` - 可选的计算单位价格设置指令
/// 
/// # 返回值
/// * `Result<()>` - 成功时返回空值，失败时返回错误
/// 
/// # 安全考虑
/// - 只有授权的资助者可以执行此操作
/// - 资助金额必须在资助者的代币账户余额内
/// - 支持Token-2022标准和转账钩子功能
/// - 资金将根据奖励持续时间进行分发
pub async fn execute_fund_reward<C: Deref<Target = impl Signer> + Clone>(
    params: FundRewardParams,
    program: &Program<C>,
    transaction_config: RpcSendTransactionConfig,
    compute_unit_price: Option<Instruction>,
) -> Result<()> {
    // 解构资助奖励参数
    let FundRewardParams {
        lb_pair,
        reward_index,
        funding_amount,
    } = params;

    let rpc_client = program.rpc();

    // 生成奖励金库的PDA，该金库存放所有奖励代币
    let (reward_vault, _bump) = derive_reward_vault_pda(lb_pair, reward_index);

    // 获取并反序列化流动性池对状态数据
    let lb_pair_state: LbPair = rpc_client
        .get_account_and_deserialize(&lb_pair, |account| {
            Ok(bytemuck::pod_read_unaligned(&account.data[8..]))     // 跳过8字节的账户判别符
        })
        .await?;

    // 获取指定索引的奖励信息和奖励代币地址
    let reward_info = lb_pair_state.reward_infos[reward_index as usize];
    let reward_mint = reward_info.mint;

    // 获取奖励代币的程序ID（SPL Token或Token-2022）
    let reward_mint_program = rpc_client.get_account(&reward_mint).await?.owner;

    // 获取或创建资助者的奖励代币关联账户
    // 该账户必须有足够的代币余额来进行资助
    let funder_token_account = get_or_create_ata(
        program,                                                    // 程序客户端
        transaction_config,                                         // 交易配置
        reward_mint,                                                // 奖励代币铸造地址
        program.payer(),                                            // 账户所有者（资助者）
        compute_unit_price.clone(),                                 // 计算单位价格
    )
    .await?;

    // 计算当前活跃箱子数组的索引和地址
    // 奖励分发需要更新活跃箱子数组的奖励信息
    let active_bin_array_idx = BinArray::bin_id_to_bin_array_index(lb_pair_state.active_id)?;
    let (bin_array, _bump) = derive_bin_array_pda(lb_pair, active_bin_array_idx as i64);

    // 生成事件权限账户PDA，用于记录资助事件
    let (event_authority, _bump) = derive_event_authority_pda();

    // 获取奖励代币的转账钩子相关账户
    // Token-2022标准支持转账钩子，可在转账时执行额外逻辑
    let reward_transfer_hook_accounts =
        get_extra_account_metas_for_transfer_hook(reward_mint, program.rpc()).await?;

    // 构建额外账户信息，用于转账钩子功能
    let remaining_accounts_info = RemainingAccountsInfo {
        slices: vec![RemainingAccountsSlice {
            accounts_type: AccountsType::TransferHookReward,        // 指定为奖励转账钩子类型
            length: reward_transfer_hook_accounts.len() as u8,      // 额外账户数量
        }],
    };

    // 构建资助奖励指令的主要账户列表
    let main_accounts = dlmm::client::accounts::FundReward {
        lb_pair,                                                    // 流动性池对账户
        reward_vault,                                               // 奖励金库账户
        reward_mint,                                                // 奖励代币铸造地址
        funder: program.payer(),                                    // 资助者账户（交易付款人）
        funder_token_account,                                       // 资助者的奖励代币账户
        bin_array,                                                  // 活跃箱子数组账户
        token_program: reward_mint_program,                         // 奖励代币程序ID
        event_authority,                                            // 事件权限账户
        program: dlmm::ID,                                          // DLMM程序ID
    }
    .to_account_metas(None);

    // 构建资助奖励指令的数据
    let data = dlmm::client::args::FundReward {
        reward_index,                                               // 奖励索引
        amount: funding_amount,                                     // 资助金额
        carry_forward: true,                                        // 是否继承之前的奖励
        remaining_accounts_info,                                    // 额外账户信息
    }
    .data();

    // 合并主要账户和转账钩子账户列表
    let accounts = [main_accounts.to_vec(), reward_transfer_hook_accounts].concat();

    // 构建完整的资助奖励指令
    let fund_reward_ix = Instruction {
        program_id: dlmm::ID,                                       // DLMM程序ID
        accounts,                                                   // 完整的账户列表
        data,                                                       // 指令数据
    };

    // 构建并发送交易请求
    let request_builder = program.request();
    let signature = request_builder
        .instruction(fund_reward_ix)                                // 添加资助奖励指令
        .send_with_spinner_and_config(transaction_config)          // 发送交易并等待确认
        .await;

    println!("Fund reward. Signature: {:#?}", signature);

    // 检查交易是否成功执行
    signature?;

    Ok(())
}
