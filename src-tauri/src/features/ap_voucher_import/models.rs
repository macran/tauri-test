// src-tauri/src/features/ap_voucher_import/models.rs
//
// AP 应付凭证导入专用的数据结构：DTO + 插入数据库用的结构体 + parser 产物。

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::models::enums::{ApInvType, TaxCertified};

// ─────────────────────────────────────────────────────────────
// AP 凭证导入：前端请求参数
// ─────────────────────────────────────────────────────────────
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ApVoucherImportRequest {
    pub file_path: String,
    pub received_date: String, // 用户在前端选择的接收日期
    pub preparer: String,      // 用户在前端输入的制表人
}

// ─────────────────────────────────────────────────────────────
// AP 凭证导入：单行错误
// ─────────────────────────────────────────────────────────────
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ApVoucherImportError {
    /// Excel 行号（从 1 开始，含表头）
    pub row: u32,
    /// Barcode（用于前端定位）
    pub barcode: Option<String>,
    /// 发票号码（用于前端定位）
    pub inv_num: Option<String>,
    /// 错误类型（用于分组展示）
    pub error_kind: String,
    /// 错误详情
    pub message: String,
}

// ─────────────────────────────────────────────────────────────
// AP 凭证导入：整体汇总结果
// ─────────────────────────────────────────────────────────────
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ApVoucherImportSummary {
    /// 新插入条数（content_fingerprint 此前不存在）
    pub inserted: u32,
    /// 凭证号被覆盖更新的条数（content_fingerprint 命中旧行，但凭证号不同——红冲重做）
    pub updated: u32,
    /// 完全重复、被忽略的条数（content_fingerprint 和凭证号都相同）
    pub skipped: u32,
    /// 错误条数
    pub error_count: u32,
    /// 新增供应商数
    pub vendor_new_count: u32,
    /// 已存在供应商数（复用）
    pub vendor_existing_count: u32,
    /// 是否成功提交
    pub committed: bool,
    /// 回滚原因
    pub rollback_reason: Option<String>,
    /// 错误详情列表
    pub errors: Vec<ApVoucherImportError>,
}

// ─────────────────────────────────────────────────────────────
// ApVoucherRow — 对应数据库 ap_invoice_vouchers 表的插入结构体
// ─────────────────────────────────────────────────────────────
#[derive(Debug, Clone)]
pub struct ApVoucherRow {
    pub header_id: Option<i64>,
    pub vendor_id: i64,
    pub barcode: Option<String>,
    pub invoice_no_display: String,
    pub entry_no: Option<String>,
    pub amount: i64, // 分
    pub tax_rate: Option<String>,
    pub tax: i64,   // 分
    pub total: i64, // 分
    pub inv_type: ApInvType,
    pub po_no: Option<String>,
    pub voucher_no: String,
    pub invoice_date: Option<String>,
    pub entry_date: Option<String>,
    pub remark: Option<String>,
    pub tax_certified: TaxCertified,
    pub received_date: String, // 前端传入
    pub preparer: String,      // 前端传入
    pub content_fingerprint: Vec<u8>,
}

// ─────────────────────────────────────────────────────────────
// 解析后的单行数据（解析阶段产物，尚未写库）
// ─────────────────────────────────────────────────────────────
#[derive(Debug, Clone)]
pub struct ParsedApVoucherRow {
    pub excel_row: u32,
    pub barcode: Option<String>,
    pub inv_code: Option<String>, // 发票代码
    pub inv_num: String,          // 发票号码（必填；收据/通知单也会有数据编码或占位文本）
    pub entry_no: Option<String>, // 入账号码
    pub amount: i64,              // 不含税金额（分）
    pub tax_rate: Option<String>, // 税率（原始字符串）
    pub tax: i64,                 // 税额（分）
    pub total: i64,               // 含税金额（分）
    pub inv_type: ApInvType,
    pub po_no: Option<String>,
    pub vendor_code: String,
    pub vendor_name: String,
    pub voucher_no: String,           // 凭证号，业务上保证非空
    pub invoice_date: Option<String>, // 开票日期
    pub entry_date: Option<String>,   // 入账日期
    pub remark: Option<String>,
    pub tax_certified: TaxCertified,
}
