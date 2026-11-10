// src-tauri/src/features/certified_import/models.rs
//
// 认证清单导入专用的数据结构：
//   - DTO（前端交互用，camelCase，标注 specta::Type）
//   - 插入数据库用的结构体
//   - Raw 结构体（直接映射 Excel 列）+ parser/repository 之间传递的中间结构体

use calamine::Data;
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::error::RowParseError;

// ─────────────────────────────────────────────
// DTO：前端交互用
// ─────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CertifiedImportSummary {
    /// 税款所属期（如 "202604"）
    pub tax_period: String,
    /// 成功插入条数
    pub inserted: u32,
    /// 解析失败条数（枚举/金额/必填字段）
    pub parse_error_count: u32,
    /// header 未找到条数（记录后跳过）
    pub header_not_found_count: u32,
    /// 重复跳过条数（header_id UNIQUE 冲突）
    pub duplicate_count: u32,
    /// 是否成功提交（false 表示已回滚）
    pub committed: bool,
    /// 回滚原因（仅 committed=false 时有值）
    pub rollback_reason: Option<String>,
    /// 解析错误详情
    pub parse_errors: Vec<RowParseError>,
    /// header 未找到的 inv_unq_key 列表
    pub header_not_found_keys: Vec<String>,
}

// ─────────────────────────────────────────────────────────────
// 插入数据库用的结构体
// ─────────────────────────────────────────────────────────────

#[allow(dead_code)] // is_travel_expense / voucher_no / posted_date 暂不插入，字段先占位
#[derive(Debug, Clone)]
pub struct CertifiedList {
    /// 关联 invoice_headers.id
    pub header_id: i64,
    /// 转内销证明编号（可为空）
    pub domestic_proof_no: Option<String>,
    /// 有效抵扣税额（分，NOT NULL）
    pub valid_deduct_amount: i64,
    /// 票种标签（NOT NULL）
    pub invoice_tag: String,
    /// 勾选时间（NOT NULL）
    pub check_time: String,
    /// 风险状态整型（NOT NULL）
    pub risk_status: u8,
    /// 税款所属期（如 "202604"）
    pub tax_period: String,
    /// 是否为差旅费（固定 0），voucher_no / posted_date 暂不插入
    pub is_travel_expense: u8,
    pub voucher_no: Option<String>,
    pub posted_date: Option<String>,
}

// ─────────────────────────────────────────────────────────────
// Raw 结构体：直接映射「发票」sheet 的 Excel 列
// ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct CertifiedRaw {
    /// 数电发票号码（用于 build_unq_key）
    pub e_num: Option<String>,
    /// 发票代码（用于 build_unq_key）
    pub code: Option<String>,
    /// 发票号码（用于 build_unq_key）
    pub num: Option<String>,
    /// 转内销证明编号（可为空）
    pub domestic_proof_no: Option<String>,
    /// 有效抵扣税额（元，字符串，后续转分）
    pub valid_deduct_amount: String,
    /// 票种标签
    pub invoice_tag: String,
    /// 勾选时间
    pub check_time: String,
    /// 风险状态原始字符串（后续枚举解析）
    pub risk_status_raw: String,
}

impl CertifiedRaw {
    pub fn from_row(row: &[Data]) -> Result<Self, (usize, ())> {
        Ok(Self {
            e_num: data_to_opt_str(row, 4),
            code: data_to_opt_str(row, 5),
            num: data_to_opt_str(row, 6),
            domestic_proof_no: data_to_opt_str(row, 3),
            valid_deduct_amount: data_to_req_str(row, 12).ok_or((12, ()))?,
            invoice_tag: data_to_req_str(row, 14).ok_or((14, ()))?,
            check_time: data_to_req_str(row, 16).ok_or((16, ()))?,
            risk_status_raw: data_to_req_str(row, 18).ok_or((18, ()))?,
        })
    }
}

/// parser 层产出、repository 层消费的中间结构体
#[derive(Debug, Clone)]
#[allow(dead_code)] // excel_row 只在构造时填充，保留给以后报错定位用
pub struct ParsedCertifiedRow {
    pub inv_unq_key: String,
    pub excel_row: u32,
    pub domestic_proof_no: Option<String>,
    pub valid_deduct_amount: i64,
    pub invoice_tag: String,
    pub check_time: String,
    pub risk_status: u8,
}

// ─────────────────────────────────────────────────────────────
// 内部辅助函数（仅供本文件使用）
// 使用 calamine::Data（0.34+）
// ─────────────────────────────────────────────────────────────

/// 取可选字符串：空/Empty 返回 None
fn data_to_opt_str(row: &[Data], idx: usize) -> Option<String> {
    match row.get(idx)? {
        Data::String(s) => {
            let t = s.trim();
            if t.is_empty() {
                None
            } else {
                Some(t.to_string())
            }
        }
        Data::Int(n) => Some(n.to_string()),
        Data::Float(f) => {
            if f.fract() == 0.0 {
                Some(format!("{}", *f as i64))
            } else {
                Some(f.to_string())
            }
        }
        Data::Bool(b) => Some(b.to_string()),
        Data::Empty => None,
        _ => None,
    }
}

/// 取必填字符串：空/Empty 返回 None，由调用方转 (usize, ())
fn data_to_req_str(row: &[Data], idx: usize) -> Option<String> {
    data_to_opt_str(row, idx)
}
