// src-tauri/src/features/travel_expense_import/models.rs
//
// Travel Expense 凭证导入专用的数据结构：DTO + parser 产物。
// 请求参数复用共享的 crate::models::dto::ImportRequest（只有 file_path 一个字段）。

use serde::{Deserialize, Serialize};
use specta::Type;

// ─────────────────────────────────────────────
// Travel Expense 凭证导入结果
// ─────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TravelExpenseImportSummary {
    /// 成功更新认证清单条数
    pub updated: u32,

    /// 错误条数
    pub error_count: u32,

    /// 是否成功提交，false 表示已回滚
    pub committed: bool,

    /// 回滚原因，仅 committed=false 时有值
    pub rollback_reason: Option<String>,

    /// 错误明细
    pub errors: Vec<TravelExpenseImportError>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TravelExpenseImportError {
    /// Excel 行号，从 1 开始
    pub row: u32,

    /// 生成出来的发票唯一标识
    pub inv_unq_key: String,

    /// 该行凭证号码
    pub voucher_no: String,

    /// 错误原因
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct ParsedTravelExpenseRow {
    pub excel_row: u32,
    pub inv_unq_key: String,
    pub voucher_no: String,
    pub posted_date: String,
}
