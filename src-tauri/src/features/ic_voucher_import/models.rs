// src-tauri/src/features/ic_voucher_import/models.rs
//
// IC 凭证导入专用的数据结构：DTO + parser 产物。

use serde::{Deserialize, Serialize};
use specta::Type;

// ─────────────────────────────────────────────────────────────
// IC 凭证导入：单行错误
// ─────────────────────────────────────────────────────────────
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct IcVoucherImportError {
    /// Excel 行号（从 1 开始）
    pub row: u32,
    /// LE
    pub legal_entity: String,
    /// 发票号码（原始值）
    pub inv_num: String,
    /// Document Number
    pub document_no: String,
    /// 错误类型（解析错误 / 发票未找到 / 税额不一致 / 价税合计不一致 / 重复数据）
    pub error_kind: String,
    /// 错误原因
    pub message: String,
}

// ─────────────────────────────────────────────────────────────
// IC 凭证导入：汇总结果
// ─────────────────────────────────────────────────────────────
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct IcVoucherImportSummary {
    /// 成功插入行数
    pub inserted: u32,
    /// 错误行数
    pub error_count: u32,
    /// 错误详情
    pub errors: Vec<IcVoucherImportError>,
    /// 是否成功提交
    pub committed: bool,
    /// 回滚原因
    pub rollback_reason: Option<String>,
}

// ─────────────────────────────────────────────────────────────
// IC 凭证导入：请求参数
// ─────────────────────────────────────────────────────────────
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct IcVoucherImportRequest {
    pub file_path: String,
    pub received_date: String,
    pub preparer: String,
}

// ─────────────────────────────────────────────────────────────
// 解析后的单行数据
// ─────────────────────────────────────────────────────────────
#[derive(Debug, Clone)]
pub struct ParsedIcVoucherRow {
    pub excel_row: u32,
    pub legal_entity: String,
    pub inv_code: Option<String>,
    pub inv_num: String, // 原始发票号码（继承后）
    pub billing: Option<String>,
    pub document_no: String,
    pub posted_date: String,
    pub original_po: Option<String>,
    /// 该行税额（分），用于汇总校验
    pub tax_fen: i64,
    /// 该行价税合计（分），用于汇总校验
    pub total_fen: i64,
}

// ─────────────────────────────────────────────────────────────
// 汇总 key：(LE, 发票号码)
// ─────────────────────────────────────────────────────────────
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct InvGroupKey {
    pub legal_entity: String,
    pub inv_num: String,
    pub inv_code: Option<String>,
}

// ─────────────────────────────────────────────────────────────
// 汇总结果：用于和 header 校验
// ─────────────────────────────────────────────────────────────
#[derive(Debug, Clone)]
pub struct InvGroupSummary {
    pub key: InvGroupKey,
    /// 该组所有行的税额之和（分）
    pub total_tax_fen: i64,
    /// 该组所有行的价税合计之和（分）
    pub total_amount_fen: i64,
    /// 该组包含的所有行（用于入库）
    pub rows: Vec<ParsedIcVoucherRow>,
}
