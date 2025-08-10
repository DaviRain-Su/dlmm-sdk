// 导入必要的依赖
use anchor_client::solana_sdk::compute_budget::ComputeBudgetInstruction;
use anchor_client::solana_sdk::instruction::Instruction;
use anchor_client::*;
use anchor_client::{
    solana_client::rpc_config::RpcSendTransactionConfig,
    solana_sdk::pubkey::Pubkey,
    solana_sdk::{
        commitment_config::CommitmentConfig,
        signer::{keypair::*, Signer},
    },
};
use anchor_lang::prelude::AccountMeta;
use anchor_lang::AccountDeserialize;
use anchor_lang::InstructionData;
use anchor_lang::ToAccountMetas;
use anyhow::*;
use clap::*;
use commons::*;
use dlmm::accounts::*;
use dlmm::types::*;
use instructions::set_pair_status_permissionless::execute_set_pair_status_permissionless;
use solana_account_decoder::*;
use std::ops::Deref;
use std::rc::Rc;
use std::time::Duration;

// 模块声明
mod args;         // 命令行参数定义
mod instructions; // 指令实现
mod math;        // 数学计算工具

use args::*;
use commons::rpc_client_extension::*;
use instructions::*;
use math::*;

/// 获取设置计算单元价格的指令
/// 用于设置交易的优先费用，提高交易被打包的概率
/// 
/// # 参数
/// * `micro_lamports` - 每个计算单元的价格（以micro lamports为单位）
/// 
/// # 返回
/// * 如果价格大于0，返回设置计算单元价格的指令
/// * 如果价格为0，返回None（不设置优先费用）
fn get_set_compute_unit_price_ix(micro_lamports: u64) -> Option<Instruction> {
    if micro_lamports > 0 {
        Some(ComputeBudgetInstruction::set_compute_unit_price(
            micro_lamports,
        ))
    } else {
        None
    }
}

/// 主函数入口
/// 使用tokio异步运行时处理所有命令
#[tokio::main]
async fn main() -> Result<()> {
    // 解析命令行参数
    let cli = Cli::parse();

    // 读取钱包密钥对文件
    let payer =
        read_keypair_file(cli.config_override.wallet).expect("Wallet keypair file not found");

    // 打印钱包公钥
    println!("Wallet {:#?}", payer.pubkey());

    // 设置确认级别为confirmed
    // confirmed表示交易已被集群中大多数节点确认
    let commitment_config = CommitmentConfig::confirmed();

    // 创建Anchor客户端，用于与Solana区块链交互
    let client = Client::new_with_options(
        cli.config_override.cluster,
        Rc::new(Keypair::from_bytes(&payer.to_bytes())?),
        commitment_config,
    );

    // 获取DLMM程序客户端
    let program = client.program(dlmm::ID)?;

    // 配置交易发送选项
    let transaction_config: RpcSendTransactionConfig = RpcSendTransactionConfig {
        skip_preflight: false,        // 不跳过预检
        preflight_commitment: Some(commitment_config.commitment), // 预检确认级别
        encoding: None,               // 使用默认编码
        max_retries: None,            // 使用默认重试次数
        min_context_slot: None,       // 不设置最小上下文槽位
    };

    // 根据用户设置创建计算单元价格指令（优先费用）
    let compute_unit_price_ix = get_set_compute_unit_price_ix(cli.config_override.priority_fee);

    // 根据用户输入的命令执行相应的操作
    match cli.command {
        // 初始化流动性对（版本2）
        DLMMCommand::InitializePair2(params) => {
            execute_initialize_lb_pair2(params, &program, transaction_config).await?;
        }
        // 初始化流动性对（版本1）
        DLMMCommand::InitializePair(params) => {
            execute_initialize_lb_pair(params, &program, transaction_config).await?;
        }
        DLMMCommand::InitializeBinArray(params) => {
            execute_initialize_bin_array(params, &program, transaction_config).await?;
        }
        DLMMCommand::InitializeBinArrayWithPriceRange(params) => {
            execute_initialize_bin_array_with_price_range(params, &program, transaction_config)
                .await?;
        }
        DLMMCommand::InitializeBinArrayWithBinRange(params) => {
            execute_initialize_bin_array_with_bin_range(params, &program, transaction_config)
                .await?;
        }
        DLMMCommand::InitializePositionWithPriceRange(params) => {
            execute_initialize_position_with_price_range(params, &program, transaction_config)
                .await?;
        }
        DLMMCommand::InitializePosition(params) => {
            execute_initialize_position(params, &program, transaction_config).await?;
        }
        DLMMCommand::AddLiquidity(params) => {
            execute_add_liquidity(params, &program, transaction_config, compute_unit_price_ix)
                .await?;
        }
        DLMMCommand::RemoveLiquidity(params) => {
            execute_remove_liquidity(params, &program, transaction_config, compute_unit_price_ix)
                .await?;
        }
        DLMMCommand::SwapExactIn(params) => {
            execute_swap(params, &program, transaction_config).await?;
        }

        DLMMCommand::ShowPair(params) => {
            execute_show_pair(params, &program).await?;
        }
        DLMMCommand::ShowPosition(params) => {
            execute_show_position(params, &program).await?;
        }
        DLMMCommand::ClaimReward(params) => {
            execute_claim_reward(params, &program, transaction_config, compute_unit_price_ix)
                .await?;
        }
        DLMMCommand::UpdateRewardDuration(params) => {
            execute_update_reward_duration(params, &program, transaction_config).await?;
        }
        DLMMCommand::UpdateRewardFunder(params) => {
            execute_update_reward_funder(params, &program, transaction_config).await?;
        }
        DLMMCommand::ClosePosition(params) => {
            execute_close_position(params, &program, transaction_config).await?;
        }
        DLMMCommand::ClaimFee(params) => {
            execute_claim_fee(params, &program, transaction_config, compute_unit_price_ix).await?;
        }
        DLMMCommand::IncreaseOracleLength(params) => {
            execute_increase_oracle_length(params, &program, transaction_config).await?;
        }
        DLMMCommand::ShowPresetParameter(params) => {
            execute_show_preset_parameters(params, &program).await?;
        }

        DLMMCommand::ListAllBinStep => {
            execute_list_all_bin_step(&program).await?;
        }
        DLMMCommand::SwapExactOut(params) => {
            execute_swap_exact_out(params, &program, transaction_config).await?;
        }
        DLMMCommand::SwapWithPriceImpact(params) => {
            execute_swap_with_price_impact(params, &program, transaction_config).await?;
        }
        DLMMCommand::InitializeCustomizablePermissionlessLbPair2(params) => {
            execute_initialize_customizable_permissionless_lb_pair2(
                params,
                &program,
                transaction_config,
                compute_unit_price_ix,
            )
            .await?;
        }
        DLMMCommand::InitializeCustomizablePermissionlessLbPair(params) => {
            execute_initialize_customizable_permissionless_lb_pair(
                params,
                &program,
                transaction_config,
                compute_unit_price_ix,
            )
            .await?;
        }
        // 由操作员播种流动性
        // 包含重试机制，用于处理网络错误或交易失败
        DLMMCommand::SeedLiquidityByOperator(params) => {
            let mut retry_count = 0;
            // 循环重试直到成功或达到最大重试次数
            while let Err(err) = execute_seed_liquidity_by_operator(
                params.clone(),
                &program,
                transaction_config,
                compute_unit_price_ix.clone(),
            )
            .await
            {
                println!("Error: {}", err);
                retry_count += 1;
                if retry_count >= params.max_retries {
                    println!("Exceeded max retries {}", params.max_retries);
                    break;
                }
                // 等待16秒后重试（约一个区块时间）
                tokio::time::sleep(Duration::from_secs(16)).await;
            }
        }
        DLMMCommand::SeedLiquiditySingleBinByOperator(params) => {
            execute_seed_liquidity_single_bin_by_operator(
                params,
                &program,
                transaction_config,
                compute_unit_price_ix,
            )
            .await?;
        }
        DLMMCommand::GetAllPositionsForAnOwner(params) => {
            execute_get_all_positions(&program, params).await?;
        }
        DLMMCommand::SetPairStatusPermissionless(params) => {
            execute_set_pair_status_permissionless(params, &program, transaction_config).await?;
        }
        DLMMCommand::SyncPrice(params) => {
            execute_sync_price(params, &program, transaction_config, compute_unit_price_ix).await?;
        }
        // 管理员命令处理
        DLMMCommand::Admin(command) => match command {
            // 初始化需要权限的流动性对
            AdminCommand::InitializePermissionPair(params) => {
                execute_initialize_permission_lb_pair(params, &program, transaction_config).await?;
            }
            AdminCommand::SetPairStatus(params) => {
                execute_set_pair_status(params, &program, transaction_config).await?;
            }
            AdminCommand::RemoveLiquidityByPriceRange(params) => {
                execute_remove_liquidity_by_price_range(
                    params,
                    &program,
                    transaction_config,
                    compute_unit_price_ix,
                )
                .await?;
            }
            AdminCommand::SetActivationPoint(params) => {
                execute_set_activation_point(params, &program, transaction_config).await?;
            }
            AdminCommand::ClosePresetParameter(params) => {
                execute_close_preset_parameter(params, &program, transaction_config).await?;
            }
            AdminCommand::InitializePresetParameter(params) => {
                execute_initialize_preset_parameter(params, &program, transaction_config).await?;
            }
            AdminCommand::WithdrawProtocolFee(params) => {
                execute_withdraw_protocol_fee(params, &program, transaction_config).await?;
            }
            AdminCommand::FundReward(params) => {
                execute_fund_reward(params, &program, transaction_config, compute_unit_price_ix)
                    .await?;
            }
            AdminCommand::InitializeReward(params) => {
                execute_initialize_reward(params, &program, transaction_config).await?;
            }
            AdminCommand::SetPreActivationSwapAddress(params) => {
                execute_set_pre_activation_swap_address(params, &program, transaction_config)
                    .await?;
            }
            AdminCommand::SetPreActivationDuration(params) => {
                execute_set_pre_activation_duration(params, &program, transaction_config).await?;
            }
            AdminCommand::InitializeTokenBadge(params) => {
                execute_initialize_token_badge(params, &program, transaction_config).await?;
            }
            AdminCommand::CreateClaimProtocolFeeOperator(params) => {
                execute_create_claim_protocol_fee_operator(params, &program, transaction_config)
                    .await?;
            }
            AdminCommand::CloseClaimProtocolFeeOperator(params) => {
                execute_close_claim_protocol_fee_operator(params, &program, transaction_config)
                    .await?;
            }
            AdminCommand::UpdateBaseFee(params) => {
                execute_update_base_fee(params, &program, transaction_config).await?;
            }
        },
    };

    Ok(())
}
