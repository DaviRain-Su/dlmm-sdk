use anchor_lang::Discriminator;
use solana_client::{
    rpc_config::{RpcAccountInfoConfig, RpcProgramAccountsConfig},
    rpc_filter::{Memcmp, RpcFilterType},
};

use crate::*;

/// 执行列出所有bin步长操作
/// Execute list all bin step operation
pub async fn execute_list_all_bin_step<C: Deref<Target = impl Signer> + Clone>(
    program: &Program<C>,
) -> Result<()> {
    let rpc_client = program.rpc();

    // 设置账户配置，不获取数据内容
    // Set account configuration, don't fetch data content
    let account_config = RpcAccountInfoConfig {
        encoding: Some(UiAccountEncoding::Base64),
        data_slice: Some(UiDataSliceConfig {
            offset: 0,
            length: 0,
        }),
        ..Default::default()
    };

    // 获取所有PresetParameter（v1）账户的密钥
    // Get all PresetParameter (v1) account keys
    let preset_parameter_keys = rpc_client
        .get_program_accounts_with_config(
            &dlmm::ID,
            RpcProgramAccountsConfig {
                filters: Some(vec![RpcFilterType::Memcmp(Memcmp::new_base58_encoded(
                    0,
                    &PresetParameter::DISCRIMINATOR,
                ))]),
                account_config: account_config.clone(),
                ..Default::default()
            },
        )
        .await?
        .into_iter()
        .map(|(key, _)| key)
        .collect::<Vec<_>>();

    // 获取所有PresetParameter2（v2）账户的密钥
    // Get all PresetParameter2 (v2) account keys
    let preset_parameter_v2_keys = rpc_client
        .get_program_accounts_with_config(
            &dlmm::ID,
            RpcProgramAccountsConfig {
                filters: Some(vec![RpcFilterType::Memcmp(Memcmp::new_base58_encoded(
                    0,
                    &PresetParameter2::DISCRIMINATOR,
                ))]),
                account_config,
                ..Default::default()
            },
        )
        .await?
        .into_iter()
        .map(|(key, _)| key)
        .collect::<Vec<_>>();

    // 合并所有版本的预设参数密钥
    // Combine all versioned preset parameter keys
    let all_versioned_keys = [preset_parameter_keys, preset_parameter_v2_keys].concat();

    // 批量处理账户，每次100个以避免RPC限制
    // Process accounts in batches of 100 to avoid RPC limits
    for keys in all_versioned_keys.chunks(100) {
        let accounts = rpc_client.get_multiple_accounts(keys).await?;
        for (key, account) in keys.iter().zip(accounts) {
            if let Some(account) = account {
                // 读取账户判别器
                // Read account discriminator
                let mut disc = [0u8; 8];
                disc.copy_from_slice(&account.data[..8]);

                // 根据账户类型解析参数
                // Parse parameters based on account type
                let (bin_step, base_factor, base_fee_power_factor) = if disc
                    == PresetParameter::DISCRIMINATOR
                {
                    let state = PresetParameter::try_deserialize(&mut account.data.as_ref())?;
                    (state.bin_step, state.base_factor, 0)
                } else if disc == PresetParameter2::DISCRIMINATOR {
                    let state: PresetParameter2 = bytemuck::pod_read_unaligned(&account.data[8..]);
                    (
                        state.bin_step,
                        state.base_factor,
                        state.base_fee_power_factor,
                    )
                } else {
                    continue;
                };

                // 计算基础费用百分比
                // Calculate base fee percentage
                let base_fee = (u128::from(bin_step)
                    * u128::from(base_factor).pow(base_fee_power_factor.into())
                    * 1000) as f64
                    / FEE_PRECISION as f64;

                println!(
                    "Preset Pubkey: {}. Bin step {}. Base fee: {}%",
                    key, bin_step, base_fee
                );
            }
        }
    }

    Ok(())
}
