// src-tauri/src/models/dto.rs
//
// 前端交互用 DTO。
// - 字段命名全部 camelCase（通过 serde rename_all 保证）
// - 所有类型均标注 specta::Type，用于自动生成 TypeScript 类型定义
// - 金额字段在 DTO 层使用 i64（分），前端自行除以 100 展示

use serde::{Deserialize, Serialize};
use specta::Type;

// ─────────────────────────────────────────────
// 导入命令的输入参数
// ─────────────────────────────────────────────
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ImportRequest {
    /// Excel 文件的完整路径
    pub file_path: String,
}
// 追加到 dto.rs
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ReversalAbnormalItem {
    pub red_inv_unq_key: String,
    pub red_line_no: u32,
    pub match_result_type: crate::commands::match_invoice::MatchResultType,
    pub match_note: Option<String>,
    pub blue_inv_unq_key: Option<String>,
    pub blue_issue_date: Option<String>,
    pub blue_seller_tax_no: Option<String>,
    pub blue_seller_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ReversalMatchSummary {
    pub batch_no: i32,
    pub ok_count: u32,
    pub blue_missing_count: u32,
    pub abnormal_count: u32,
    pub abnormal_items: Vec<ReversalAbnormalItem>,
}
