// src-tauri/src/models/ap_voucher.rs
//
// AP 应付凭证相关的枚举、数据库结构体。

use crate::models::enums::{ApInvType, TaxCertified};
// use serde::{Deserialize, Serialize};
// use specta::Type;
// use strum::{Display, EnumString};
// ─────────────────────────────────────────────────────────────

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
