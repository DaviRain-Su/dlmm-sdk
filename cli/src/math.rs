// DLMM数学计算工具模块
// 提供价格、bin ID、费率等相关的数学计算功能

use anyhow::{anyhow, Result};
use commons::dlmm::types::Rounding;
use commons::{BASIS_POINT_MAX, SCALE_OFFSET};
use rust_decimal::MathematicalOps;
use rust_decimal::{
    prelude::{FromPrimitive, ToPrimitive},
    Decimal,
};

/// 从费率基点计算基础因子
/// 
/// # 参数
/// * `bin_step` - bin步长（以基点为单位）
/// * `fee_bps` - 费率（以基点为单位，1基点 = 0.01%）
/// 
/// # 返回
/// * `(base_factor, power_factor)` - 基础因子和幂次因子
/// 
/// # 计算公式
/// base_factor = fee_bps * 10000 / bin_step
/// 如果结果超过u16最大值，则通过除以10的幂次来缩小
pub fn compute_base_factor_from_fee_bps(bin_step: u16, fee_bps: u16) -> Result<(u16, u8)> {
    // 计算基础因子：费率 * 10000 / bin步长
    let computed_base_factor = fee_bps as f64 * 10_000.0f64 / bin_step as f64;

    if computed_base_factor > u16::MAX as f64 {
        // 如果超过u16最大值，需要通过除以10的幂次来缩小
        let mut truncated_base_factor = computed_base_factor;
        let mut base_power_factor = 0u8;
        loop {
            if truncated_base_factor < u16::MAX as f64 {
                break;
            }

            let remainder = truncated_base_factor % 10.0;
            if remainder == 0.0 {
                // 可以整除10，增加幂次并缩小因子
                base_power_factor += 1;
                truncated_base_factor /= 10.0;
            } else {
                // 不能整除10，说明有小数部分
                return Err(anyhow!("have decimals"));
            }
        }

        Ok((truncated_base_factor as u16, base_power_factor))
    } else {
        // 合理性检查
        let casted_base_factor = computed_base_factor as u16 as f64;
        if casted_base_factor != computed_base_factor {
            if casted_base_factor == u16::MAX as f64 {
                return Err(anyhow!("overflow"));
            }

            if casted_base_factor == 0.0f64 {
                return Err(anyhow!("underflow"));
            }

            if computed_base_factor.fract() != 0.0 {
                return Err(anyhow!("have decimals"));
            }

            return Err(anyhow!("unknown error"));
        }

        Ok((computed_base_factor as u16, 0u8))
    }
}

/// 从价格精确计算bin ID
/// 只有当价格恰好对应某个bin时才返回ID，否则返回None
/// 
/// # 参数
/// * `bin_step` - bin步长（以基点为单位）
/// * `price` - 价格（每个lamport的价格）
/// 
/// # 返回
/// * 如果价格恰好对应某个bin，返回bin ID
/// * 如果价格在两个bin之间，返回None
/// 
/// # 计算公式
/// id = log(price) / log(1 + bin_step/10000)
pub fn get_precise_id_from_price(bin_step: u16, price: &Decimal) -> Option<i32> {
    // 将bin步长从基点转换为比率
    let bps = Decimal::from_u16(bin_step)?.checked_div(Decimal::from_i32(BASIS_POINT_MAX)?)?;
    // 计算基数：1 + bin_step比率
    let base = Decimal::ONE.checked_add(bps)?;

    // 使用对数公式计算bin ID
    let id = price.log10().checked_div(base.log10())?.to_f64()?;
    let trimmed_id = id as i32;
    let trimmed_id_f64 = trimmed_id as f64;

    // 检查是否为精确的整数ID
    if trimmed_id_f64 == id {
        id.to_i32()
    } else {
        None
    }
}

/// 根据价格计算bin ID，支持向上或向下舍入
/// 如果价格在两个bin之间，根据舍入模式决定返回哪个bin
/// 
/// # 参数
/// * `bin_step` - bin步长（以基点为单位）
/// * `price` - 价格（每个lamport的价格）
/// * `rounding` - 舍入模式（向上或向下）
/// 
/// # 返回
/// * 舍入后的bin ID
pub fn get_id_from_price(bin_step: u16, price: &Decimal, rounding: Rounding) -> Option<i32> {
    // 将bin步长从基点转换为比率
    let bps = Decimal::from_u16(bin_step)?.checked_div(Decimal::from_i32(BASIS_POINT_MAX)?)?;
    // 计算基数：1 + bin_step比率
    let base = Decimal::ONE.checked_add(bps)?;

    // 根据舍入模式计算bin ID
    let id = match rounding {
        Rounding::Down => price.log10().checked_div(base.log10())?.floor(), // 向下舍入
        Rounding::Up => price.log10().checked_div(base.log10())?.ceil(),    // 向上舍入
    };

    id.to_i32()
}

/// 将Q64xQ64格式的价格转换为可读的十进制数
/// Q64xQ64是一种定点数表示法，用于在智能合约中表示价格
/// 
/// # 参数
/// * `q64x64_price` - Q64xQ64格式的价格（128位整数）
/// 
/// # 返回
/// * 转换后的十进制价格（每个lamport的价格）
/// 
/// # 说明
/// Q64xQ64格式使用64位表示整数部分，64位表示小数部分
pub fn q64x64_price_to_decimal(q64x64_price: u128) -> Option<Decimal> {
    let q_price = Decimal::from_u128(q64x64_price)?;
    // SCALE_OFFSET通常是64，表示需要除以2^64来获得实际价格
    let scale_off = Decimal::TWO.powu(SCALE_OFFSET.into());
    q_price.checked_div(scale_off)
}

/// 将每个代币的价格转换为每个lamport的价格
/// 
/// # 参数
/// * `price_per_token` - 每个代币的价格（UI价格）
/// * `base_token_decimal` - 基础代币的小数位数
/// * `quote_token_decimal` - 报价代币的小数位数
/// 
/// # 返回
/// * 每个lamport的价格
/// 
/// # 计算公式
/// price_per_lamport = price_per_token * 10^quote_decimal / 10^base_decimal
/// 
/// # 示例
/// 如果BTC/USDC价格是30000（每个BTC价值30000 USDC）
/// BTC有9位小数，USDC有6位小数
/// 则每个lamport BTC的价格 = 30000 * 10^6 / 10^9 = 0.03 USDC lamport
pub fn price_per_token_to_per_lamport(
    price_per_token: f64,
    base_token_decimal: u8,
    quote_token_decimal: u8,
) -> Option<Decimal> {
    let price_per_token = Decimal::from_f64(price_per_token)?;
    price_per_token
        .checked_mul(Decimal::TEN.powu(quote_token_decimal.into()))?
        .checked_div(Decimal::TEN.powu(base_token_decimal.into()))
}

/// 将每个lamport的价格转换为每个代币的价格（UI价格）
/// 
/// # 参数
/// * `price_per_lamport` - 每个lamport的价格
/// * `base_token_decimal` - 基础代币的小数位数
/// * `quote_token_decimal` - 报价代币的小数位数
/// 
/// # 返回
/// * 每个代币的价格（UI价格）
/// 
/// # 计算公式
/// price_per_token = price_per_lamport * 10^base_decimal / 10^quote_decimal
/// 
/// # 示例
/// 如果每个BTC lamport价格是0.03 USDC lamport
/// BTC有9位小数，USDC有6位小数
/// 则每个BTC的价格 = 0.03 * 10^9 / 10^6 = 30000 USDC
pub fn price_per_lamport_to_price_per_token(
    price_per_lamport: f64,
    base_token_decimal: u8,
    quote_token_decimal: u8,
) -> Option<Decimal> {
    // 一个完整代币的lamport数量
    let one_ui_base_token_amount = Decimal::TEN.powu(base_token_decimal.into());
    // 一个完整报价代币的lamport数量
    let one_ui_quote_token_amount = Decimal::TEN.powu(quote_token_decimal.into());
    let price_per_lamport = Decimal::from_f64(price_per_lamport)?;

    // 转换公式：UI价格 = lamport价格 * base代币lamports / quote代币lamports
    one_ui_base_token_amount
        .checked_mul(price_per_lamport)?
        .checked_div(one_ui_quote_token_amount)
}
