use crate::*;
use anchor_lang::Discriminator;

/// 关闭预设参数账户的参数结构体
/// 该操作将删除不再需要的预设参数账户并回收租金
/// 只能关闭没有被任何池对使用的预设参数
#[derive(Debug, Parser)]
pub struct ClosePresetAccountParams {
    /// 预设参数的公钥地址，可以通过ListAllBinStep命令获取
    /// 该参数必须是现有的且没有被使用的预设参数
    pub preset_parameter: Pubkey,
}

/// 执行关闭预设参数操作
/// 
/// 此函数删除不再需要的预设参数账户，回收租金以节约成本。
/// 这是一个重要的资源管理功能，用于清理不再使用的配置。
pub async fn execute_close_preset_parameter<C: Deref<Target = impl Signer> + Clone>(
    params: ClosePresetAccountParams,
    program: &Program<C>,
    transaction_config: RpcSendTransactionConfig,
) -> Result<Pubkey> {
    // 解构参数，获取要关闭的预设参数地址
    let ClosePresetAccountParams { preset_parameter } = params;

    let rpc_client = program.rpc();
    // 获取预设参数账户数据以确定其类型
    let preset_parameter_account = rpc_client.get_account(&preset_parameter).await?;

    // 提取账户判别符，用于确定是哪个版本的预设参数
    let disc = &preset_parameter_account.data[..8];

    // 根据账户判别符构建相应的关闭指令
    let instruction = if disc == dlmm::accounts::PresetParameter::DISCRIMINATOR {
        // 处理第一版预设参数
        let accounts = dlmm::client::accounts::ClosePresetParameter {
            admin: program.payer(),                                 // 管理员账户
            rent_receiver: program.payer(),                         // 租金接收者
            preset_parameter,                                       // 要关闭的预设参数
        }
        .to_account_metas(None);

        let data = dlmm::client::args::ClosePresetParameter {}.data();

        Instruction {
            program_id: dlmm::ID,
            accounts,
            data,
        }
    } else if disc == dlmm::accounts::PresetParameter2::DISCRIMINATOR {
        // 处理第二版预设参数
        let accounts = dlmm::client::accounts::ClosePresetParameter2 {
            admin: program.payer(),                                 // 管理员账户
            rent_receiver: program.payer(),                         // 租金接收者
            preset_parameter,                                       // 要关闭的预设参数
        }
        .to_account_metas(None);

        let data = dlmm::client::args::ClosePresetParameter2 {}.data();

        Instruction {
            program_id: dlmm::ID,
            accounts,
            data,
        }
    } else {
        bail!("Not a valid preset parameter account");              // 不是有效的预设参数账户
    };

    // 构建并发送交易请求
    let request_builder = program.request();
    let signature = request_builder
        .instruction(instruction)                                   // 添加关闭指令
        .send_with_spinner_and_config(transaction_config)          // 发送交易并等待确认
        .await;

    println!(
        "Close preset parameter {}. Signature: {signature:#?}",
        preset_parameter
    );

    // 检查交易是否成功执行
    signature?;

    // 返回已关闭的预设参数地址
    Ok(preset_parameter)
}
