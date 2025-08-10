/// DLMM CLI 指令模块
/// 包含所有DLMM（Dynamic Liquidity Market Maker）协议的CLI指令实现
/// DLMM CLI instruction modules
/// Contains all CLI instruction implementations for DLMM (Dynamic Liquidity Market Maker) protocol

// === 流动性管理 / Liquidity Management ===

/// 添加流动性指令 / Add liquidity instruction
pub mod add_liquidity;
pub use add_liquidity::*;

/// 移除流动性指令 / Remove liquidity instruction
pub mod remove_liquidity;
pub use remove_liquidity::*;

// === 费用和奖励 / Fees and Rewards ===

/// 申领费用指令 / Claim fee instruction
pub mod claim_fee;
pub use claim_fee::*;

/// 申领奖励指令 / Claim reward instruction
pub mod claim_reward;
pub use claim_reward::*;

/// 注资奖励指令 / Fund reward instruction
pub mod fund_reward;
pub use fund_reward::*;

// === 头寸管理 / Position Management ===

/// 关闭头寸指令 / Close position instruction
pub mod close_position;
pub use close_position::*;

/// 获取所有头寸指令 / Get all positions instruction
pub mod get_all_positions;
pub use get_all_positions::*;

/// 初始化头寸指令 / Initialize position instruction
pub mod initialize_position;
pub use initialize_position::*;

/// 按价格范围初始化头寸指令 / Initialize position with price range instruction
pub mod initialize_position_with_price_range;
pub use initialize_position_with_price_range::*;

/// 显示头寸指令 / Show position instruction
pub mod show_position;
pub use show_position::*;

// === 预言机管理 / Oracle Management ===

/// 增加预言机长度指令 / Increase oracle length instruction
pub mod increase_oracle_length;
pub use increase_oracle_length::*;

// === Bin数组管理 / Bin Array Management ===

/// 初始化bin数组指令 / Initialize bin array instruction
pub mod initialize_bin_array;
pub use initialize_bin_array::*;

/// 按bin范围初始化bin数组指令 / Initialize bin array with bin range instruction
pub mod initialize_bin_array_with_bin_range;
pub use initialize_bin_array_with_bin_range::*;

/// 按价格范围初始化bin数组指令 / Initialize bin array with price range instruction
pub mod initialize_bin_array_with_price_range;
pub use initialize_bin_array_with_price_range::*;

// === 流动性对管理 / Liquidity Pair Management ===

/// 初始化可定制无权限流动性对指令v2 / Initialize customizable permissionless LB pair instruction v2
pub mod initialize_customizable_permissionless_lb_pair2;
pub use initialize_customizable_permissionless_lb_pair2::*;

/// 初始化可定制无权限流动性对指令 / Initialize customizable permissionless LB pair instruction
pub mod initialize_customizable_permissionless_lb_pair;
pub use initialize_customizable_permissionless_lb_pair::*;

/// 初始化流动性对指令v2 / Initialize LB pair instruction v2
pub mod initialize_lb_pair2;
pub use initialize_lb_pair2::*;

/// 初始化流动性对指令 / Initialize LB pair instruction
pub mod initialize_lb_pair;
pub use initialize_lb_pair::*;

/// 显示流动性对指令 / Show pair instruction
pub mod show_pair;
pub use show_pair::*;

/// 同步价格指令 / Sync price instruction
pub mod sync_price;
pub use sync_price::*;

// === 交易功能 / Trading Functions ===

/// 精确输入交换指令 / Swap exact in instruction
pub mod swap_exact_in;
pub use swap_exact_in::*;

/// 精确输出交换指令 / Swap exact out instruction
pub mod swap_exact_out;
pub use swap_exact_out::*;

/// 按价格影响交换指令 / Swap with price impact instruction
pub mod swap_with_price_impact;
pub use swap_with_price_impact::*;

// === 查询和显示 / Query and Display ===

/// 列出所有bin步长指令 / List all bin step instruction
pub mod list_all_binstep;
pub use list_all_binstep::*;

/// 显示预设参数指令 / Show preset parameters instruction
pub mod show_preset_parameters;
pub use show_preset_parameters::*;

// === 状态管理 / Status Management ===

/// 设置流动性对状态（无权限）/ Set pair status (permissionless)
pub mod set_pair_status_permissionless;

// === 管理员功能 / Admin Functions ===

/// 管理员指令模块 / Admin instruction modules
pub mod admin;
pub use admin::*;

// === 初始流动性管理 / Initial Liquidity Management ===

/// ILM指令模块 / ILM instruction modules
pub mod ilm;
pub use ilm::*;

// === 通用工具 / Common Utils ===

/// 通用工具函数 / Common utility functions
mod utils;
pub use utils::*;
