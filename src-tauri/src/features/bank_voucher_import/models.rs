// src-tauri/src/features/bank_voucher_import/models.rs
//
// Bank 进项发票凭证导入专用的数据结构：DTO + parser 产物。

use serde::{Deserialize, Serialize};
use specta::Type;

// ─────────────────────────────────────────────
// Bank 进项发票凭证导入
// ─────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BankVoucherImportRequest {
    /// Excel 文件完整路径
    pub file_path: String,

    /// 收到日期，由前端日期选择器传入，建议格式 YYYY-MM-DD
    pub received_date: String,

    /// 制表人，由前端 input 输入
    pub prepared_by: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum BankVoucherErrorKind {
    /// Excel 行本身解析错误，例如发票号码为空、税额格式错误、凭证号码为空
    Parse,

    /// invoice_headers 中找不到对应发票
    NotFound,

    /// 找到了发票，但 Excel 税额和 invoice_headers.tax 不一致
    TaxMismatch,

    /// 按发票号码匹配到了多张发票，无法唯一确定 header_id
    Ambiguous,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BankVoucherImportError {
    /// Excel 行号，从 1 开始，包含表头行
    pub row: u32,

    /// 发票号码，来自 Bank Excel D 列
    pub invoice_no: String,

    /// 凭证号码，来自 Bank Excel I 列
    pub voucher_no: String,

    /// 错误类型
    pub kind: BankVoucherErrorKind,

    /// 错误描述
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BankVoucherImportSummary {
    /// 成功插入或更新的记录数
    pub inserted: u32,

    /// invoice_headers 中找不到对应发票的数量
    pub not_found_count: u32,

    /// 税额不一致的数量
    pub tax_mismatch_count: u32,

    /// 匹配到多张发票的数量
    pub ambiguous_count: u32,

    /// Excel 解析错误数量
    pub parse_error_count: u32,

    /// 总错误数量
    pub error_count: u32,

    /// 是否成功提交数据库事务
    pub committed: bool,

    /// 如果回滚，说明原因
    pub rollback_reason: Option<String>,

    /// 错误明细
    pub errors: Vec<BankVoucherImportError>,
}

#[derive(Debug, Clone)]
pub struct ParsedBankVoucherRow {
    pub excel_row: u32,
    pub invoice_no: String,
    pub voucher_no: String,
    pub posted_date: String,
    pub tax_fen: i64,
}
