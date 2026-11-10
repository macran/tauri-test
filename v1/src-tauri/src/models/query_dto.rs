// src-tauri/src/models/query_dto.rs
//
// 各查询页面用的请求 / 行 DTO。
// 命名约定：
//   - XxxFilter   前端查询表单提交的筛选条件，字段均为 Option
//   - XxxRow      查询结果行，包含该表全部字段 + 便于用户查询的关联字段
//                 （如 invoice_number / seller_name，用户不关心外键 header_id）
//   - PageResult<T> 统一分页返回结构

use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PageResult<T> {
    pub rows: Vec<T>,
    pub total: u32,
}

// ─────────────────────────────────────────────────────────────
// 发票头
// ─────────────────────────────────────────────────────────────
#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct InvoiceHeaderFilter {
    pub invoice_number: Option<String>,
    pub seller_name: Option<String>,
    pub seller_tax_no: Option<String>,
    pub issue_date_from: Option<String>,
    pub issue_date_to: Option<String>,
    pub inv_type: Option<Vec<u8>>,
    pub state: Option<Vec<u8>>,
    pub is_pos: Option<Vec<u8>>,
    pub risk: Option<Vec<u8>>,
    pub source: Option<Vec<u8>>,
    pub remark: Option<String>,
    pub batch_no: Option<i32>,
    pub rev_org_key: Option<String>,
    pub red_notice_no: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct InvoiceHeaderRow {
    pub id: u32,
    pub code: Option<String>,
    pub num: Option<String>,
    pub e_num: Option<String>,
    pub inv_unq_key: String,
    pub seller_id: u32,
    pub seller_tax_no: String,
    pub seller_name: String,
    pub issue_date: String,
    pub amount: String,
    pub tax: String,
    pub total: String,
    pub source: u8,
    pub inv_type: u8,
    pub state: u8,
    pub is_pos: u8,
    pub risk: u8,
    pub issuer: Option<String>,
    pub remark: Option<String>,
    pub batch_no: i32,
    pub rev_org_key: Option<String>,
    pub red_notice_no: Option<String>,
}

/// 明细类查询行附带的发票头信息（各明细表展示的前 13 列）
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct InvoiceHead {
    pub code: Option<String>,
    pub num: Option<String>,
    pub e_num: Option<String>,
    pub seller_tax_no: String,
    pub seller_name: String,
    pub issue_date: String,
    pub source: u8,
    pub inv_type: u8,
    pub state: u8,
    pub is_pos: u8,
    pub risk: u8,
    pub issuer: Option<String>,
    pub remark: Option<String>,
}

// ─────────────────────────────────────────────────────────────
// 发票明细
// ─────────────────────────────────────────────────────────────
#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct InvoiceDetailFilter {
    pub invoice_number: Option<String>,
    pub seller_name: Option<String>,
    pub goods_name: Option<String>,
    pub goods_category: Option<String>,
    pub spec_model: Option<String>,
    pub unit: Option<String>,
    pub tax_code: Option<String>,
    pub biz_type: Option<Vec<u8>>,
    pub issue_date_from: Option<String>,
    pub issue_date_to: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct InvoiceDetailRow {
    pub id: u32,
    pub header_id: u32,
    pub line_no: u32,
    pub head: InvoiceHead,
    pub tax_code: Option<String>,
    pub biz_type: Option<u8>,
    pub goods_category: Option<String>,
    pub goods_name: String,
    pub spec_model: Option<String>,
    pub unit: Option<String>,
    pub quantity: Option<String>,
    pub unit_price: Option<String>,
    pub amount: String,
    pub tax_rate: Option<String>,
    pub tax: String,
    pub total: String,
}

// ─────────────────────────────────────────────────────────────
// 7 个特定行业明细表
// ─────────────────────────────────────────────────────────────
#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BuildingDetailFilter {
    pub invoice_number: Option<String>,
    pub seller_name: Option<String>,
    pub project_name: Option<String>,
    pub service_location: Option<String>,
    pub cross_region_no: Option<String>,
    pub issue_date_from: Option<String>,
    pub issue_date_to: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BuildingDetailRow {
    pub id: u32,
    pub header_id: u32,
    pub line_no: u32,
    pub head: InvoiceHead,
    pub cross_region_no: Option<String>,
    pub service_location: Option<String>,
    pub project_name: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RealEstateDetailFilter {
    pub invoice_number: Option<String>,
    pub seller_name: Option<String>,
    pub plate_no: Option<String>,
    pub cert_no: Option<String>,
    pub issue_date_from: Option<String>,
    pub issue_date_to: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RealEstateDetailRow {
    pub id: u32,
    pub header_id: u32,
    pub line_no: u32,
    pub head: InvoiceHead,
    pub cert_no: Option<String>,
    pub plate_no: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PassengerDetailFilter {
    pub invoice_number: Option<String>,
    pub seller_name: Option<String>,
    pub passenger_name: Option<String>,
    pub id_no: Option<String>,
    pub depart: Option<String>,
    pub arrival: Option<String>,
    pub transport_type: Option<String>,
    pub travel_date_from: Option<String>,
    pub travel_date_to: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PassengerDetailRow {
    pub id: u32,
    pub header_id: u32,
    pub line_no: u32,
    pub head: InvoiceHead,
    pub passenger_name: Option<String>,
    pub id_no: Option<String>,
    pub travel_date: Option<String>,
    pub depart: Option<String>,
    pub arrival: Option<String>,
    pub seat_class: Option<String>,
    pub transport_type: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CargoDetailFilter {
    pub invoice_number: Option<String>,
    pub seller_name: Option<String>,
    pub vehicle_no: Option<String>,
    pub cargo_name: Option<String>,
    pub depart: Option<String>,
    pub arrival: Option<String>,
    pub issue_date_from: Option<String>,
    pub issue_date_to: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CargoDetailRow {
    pub id: u32,
    pub header_id: u32,
    pub line_no: u32,
    pub head: InvoiceHead,
    pub transport_type: Option<String>,
    pub vehicle_no: Option<String>,
    pub depart: Option<String>,
    pub arrival: Option<String>,
    pub cargo_name: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AirDetailFilter {
    pub invoice_number: Option<String>,
    pub seller_name: Option<String>,
    pub passenger_name: Option<String>,
    pub id_no: Option<String>,
    pub flight_no: Option<String>,
    pub depart: Option<String>,
    pub arrival: Option<String>,
    pub flight_date_from: Option<String>,
    pub flight_date_to: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AirDetailRow {
    pub id: u32,
    pub header_id: u32,
    pub line_no: u32,
    pub head: InvoiceHead,
    pub passenger_name: Option<String>,
    pub id_no: Option<String>,
    pub depart: Option<String>,
    pub arrival: Option<String>,
    pub carrier: Option<String>,
    pub flight_no: Option<String>,
    pub flight_date: Option<String>,
    pub flight_time: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RailwayDetailFilter {
    pub invoice_number: Option<String>,
    pub seller_name: Option<String>,
    pub passenger_name: Option<String>,
    pub id_no: Option<String>,
    pub train_no: Option<String>,
    pub depart: Option<String>,
    pub arrival: Option<String>,
    pub travel_date_from: Option<String>,
    pub travel_date_to: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RailwayDetailRow {
    pub id: u32,
    pub header_id: u32,
    pub line_no: u32,
    pub head: InvoiceHead,
    pub passenger_name: Option<String>,
    pub id_no: Option<String>,
    pub depart: Option<String>,
    pub arrival: Option<String>,
    pub train_no: Option<String>,
    pub travel_date: Option<String>,
    pub travel_time: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TollDetailFilter {
    pub invoice_number: Option<String>,
    pub seller_name: Option<String>,
    pub plate_no: Option<String>,
    pub vehicle_type: Option<String>,
    pub toll_date_from: Option<String>,
    pub toll_date_to: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TollDetailRow {
    pub id: u32,
    pub header_id: u32,
    pub line_no: u32,
    pub head: InvoiceHead,
    pub plate_no: Option<String>,
    pub vehicle_type: Option<String>,
    pub toll_date_from: Option<String>,
    pub toll_date_to: Option<String>,
}

// ─────────────────────────────────────────────────────────────
// 认证清单 / 差旅费用（共用 certified_list 表，按 is_travel_expense 区分）
// ─────────────────────────────────────────────────────────────
#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CertifiedListFilter {
    pub invoice_number: Option<String>,
    pub seller_name: Option<String>,
    pub tax_period: Option<String>,
    pub risk_status: Option<Vec<u8>>,
    pub domestic_proof_no: Option<String>,
    pub invoice_tag: Option<String>,
    pub check_time_from: Option<String>,
    pub check_time_to: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CertifiedListRow {
    pub id: u32,
    pub header_id: u32,
    pub invoice_number: String,
    pub issue_date: String,
    pub seller_name: String,
    pub amount: String,
    pub tax: String,
    pub total: String,
    pub domestic_proof_no: Option<String>,
    pub valid_deduct_amount: String,
    pub invoice_tag: String,
    pub check_time: String,
    pub risk_status: u8,
    pub tax_period: Option<String>,
    pub is_travel_expense: u8,
    pub voucher_no: Option<String>,
    pub posted_date: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TravelExpenseFilter {
    pub invoice_number: Option<String>,
    pub seller_name: Option<String>,
    pub voucher_no: Option<String>,
    pub posted_date_from: Option<String>,
    pub posted_date_to: Option<String>,
    pub tax_period: Option<String>,
    pub risk_status: Option<Vec<u8>>,
}

// ─────────────────────────────────────────────────────────────
// 银行回单凭证
// ─────────────────────────────────────────────────────────────
#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BankVoucherFilter {
    pub invoice_number: Option<String>,
    pub seller_name: Option<String>,
    pub voucher_no: Option<String>,
    pub posted_date_from: Option<String>,
    pub posted_date_to: Option<String>,
    pub received_date_from: Option<String>,
    pub received_date_to: Option<String>,
    pub prepared_by: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BankVoucherRow {
    pub id: u32,
    pub header_id: u32,
    pub invoice_number: String,
    pub issue_date: String,
    pub seller_name: String,
    pub amount: String,
    pub tax: String,
    pub total: String,
    pub voucher_no: String,
    pub posted_date: String,
    pub received_date: String,
    pub prepared_by: Option<String>,
}

// ─────────────────────────────────────────────────────────────
// AP 应付凭证
// ─────────────────────────────────────────────────────────────
#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ApVoucherFilter {
    pub invoice_no_display: Option<String>,
    pub vendor_name: Option<String>,
    pub vendor_code: Option<String>,
    pub voucher_no: Option<String>,
    pub po_no: Option<String>,
    pub barcode: Option<String>,
    pub entry_no: Option<String>,
    pub inv_type: Option<Vec<u8>>,
    pub tax_certified: Option<Vec<u8>>,
    pub entry_date_from: Option<String>,
    pub entry_date_to: Option<String>,
    pub invoice_date_from: Option<String>,
    pub invoice_date_to: Option<String>,
    pub received_date_from: Option<String>,
    pub received_date_to: Option<String>,
    pub preparer: Option<String>,
    pub remark: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ApVoucherRowDto {
    pub id: u32,
    pub header_id: Option<u32>,
    pub invoice_number: Option<String>,
    pub vendor_id: u32,
    pub vendor_code: String,
    pub vendor_name: String,
    pub barcode: Option<String>,
    pub invoice_no_display: String,
    pub entry_no: Option<String>,
    pub amount: String,
    pub tax_rate: Option<String>,
    pub tax: String,
    pub total: String,
    pub inv_type: u8,
    pub po_no: Option<String>,
    pub voucher_no: String,
    pub invoice_date: Option<String>,
    pub entry_date: Option<String>,
    pub remark: Option<String>,
    pub tax_certified: u8,
    pub received_date: String,
    pub preparer: String,
}

// ─────────────────────────────────────────────────────────────
// IC 凭证
// ─────────────────────────────────────────────────────────────
#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct IcVoucherFilter {
    pub invoice_number: Option<String>,
    pub document_no: Option<String>,
    pub legal_entity: Option<String>,
    pub seller_name: Option<String>,
    pub original_po: Option<String>,
    pub billing: Option<String>,
    pub posted_date_from: Option<String>,
    pub posted_date_to: Option<String>,
    pub received_date_from: Option<String>,
    pub received_date_to: Option<String>,
    pub preparer: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct IcVoucherRow {
    pub id: u32,
    pub header_id: u32,
    pub invoice_number: String,
    pub seller_name: String,
    pub legal_entity: String,
    pub billing: Option<String>,
    pub document_no: String,
    pub posted_date: Option<String>,
    pub original_po: Option<String>,
    pub received_date: String,
    pub preparer: String,
}

// ─────────────────────────────────────────────────────────────
// 凭证编辑（bank / AP / IC）：只包含允许修改的字段
// ─────────────────────────────────────────────────────────────
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BankVoucherEdit {
    pub id: u32,
    pub voucher_no: String,
    pub posted_date: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ApVoucherEdit {
    pub id: u32,
    pub barcode: Option<String>,
    pub entry_no: Option<String>,
    pub po_no: Option<String>,
    pub vendor_code: String,
    pub vendor_name: String,
    pub voucher_no: String,
    pub entry_date: Option<String>,
    pub remark: Option<String>,
    pub tax_certified: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct IcVoucherEdit {
    pub id: u32,
    pub billing: Option<String>,
    pub document_no: String,
    pub posted_date: Option<String>,
    pub original_po: Option<String>,
}

// ─────────────────────────────────────────────────────────────
// 综合查询（发票明细 + 所有关联记录）
// 一行 = 一个发票明细 item。字段很多且随列选择变化，所以不逐个建 struct：
// 记录用「列名 → 文本值」的 map，列定义（标签/类型）只在前端维护。
// 金额均为分的文本，枚举为数字文本。
// ─────────────────────────────────────────────────────────────
pub type Rec = std::collections::HashMap<String, Option<String>>;

#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ItemOverviewFilter {
    pub issue_date_from: Option<String>,
    pub issue_date_to: Option<String>,
    pub invoice_number: Option<String>,
    /// 销方名称或税号
    pub seller: Option<String>,
    pub goods_name: Option<String>,
    pub tax_code: Option<String>,
    pub biz_type: Option<Vec<u8>>,
    /// AP / IC / Bank / 差旅 任一凭证号
    pub voucher_no: Option<String>,
    /// 1 = 已勾选，0 = 未勾选
    pub certified: Option<Vec<u8>>,
    /// 下面两项须由同一条 AP 记录同时满足
    pub ap_voucher_no: Option<String>,
    pub ap_vendor_code: Option<String>,
    /// 1 = 作为红字明细，2 = 作为蓝字被冲，3 = 无红冲匹配
    pub reversal: Option<Vec<u8>>,
    /// 税务计算期间（YYYYMM，闭区间）；税务列与下面两个筛选都依赖它
    pub period_from: Option<String>,
    pub period_to: Option<String>,
    /// 进项税转出：1 = 转出，0 = 不转出
    pub transfer: Option<Vec<u8>>,
    /// 印花税：1 = 计入，2 = 计入但待确认类别，0 = 不计入
    pub stamp: Option<Vec<u8>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ItemOverviewRow {
    /// item + 发票头 + 销方 + 认证 + Bank + 税务结论（单值）
    pub item: Rec,
    /// 多值记录组：ap / ic / sp{biz_type} / rev / wth / sth
    pub groups: std::collections::HashMap<String, Vec<Rec>>,
}
