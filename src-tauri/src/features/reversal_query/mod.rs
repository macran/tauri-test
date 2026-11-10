// 红冲查询：一次查询返回汇总表、明细表和对应关系。
//   - 蓝字视角：汇总 = 蓝字发票一行，明细 = 蓝字明细一行
//   - 红字视角：汇总 = 红字发票一行，明细 = 红字明细一行（含未核对的红票）
//   - 对应关系：一行 = 一条红冲匹配记录，两个视角共用
pub mod commands;
pub mod models;
pub mod red;
pub mod repository;

pub use commands::*;
pub use models::*;
pub use repository::{discount_flags, DiscountInput};
