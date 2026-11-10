// src-tauri/src/parser/mod.rs
//
// 每个模块对应一个 Excel sheet 的解析逻辑。
// 所有模块均返回各自的 XxxParseResult，包含：
//   - rows:   解析成功的行
//   - errors: 解析失败的行（带行号+列名+错误描述）

pub mod detail; // 信息汇总表 sheet
pub mod header; // 发票基础信息 sheet

pub mod ap_voucher;
pub mod bank_voucher;
pub mod certified;
pub mod ic_voucher;
pub mod special; // 发票认证信息 sheet
pub mod travel_expense; // 差旅费发票 sheet // AP Voucher sheet // IC Voucher sheet
