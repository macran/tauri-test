// src-tauri/src/models/dto.rs
//
// 前端交互用 DTO。
// - 字段命名全部 camelCase（通过 serde rename_all 保证）
// - 所有类型均标注 specta::Type，用于自动生成 TypeScript 类型定义
// - 金额字段在 DTO 层使用 i64（分），前端自行除以 100 展示

use crate::error::RowParseError;
use serde::{Deserialize, Serialize};
use specta::Type;
// ─────────────────────────────────────────────
// 单个错误信息（定位到具体单元格）
// ─────────────────────────────────────────────
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CellError {
    /// Excel 行号（从 1 开始，包含表头行）
    pub row: u32,
    /// 列名称（中文列名）
    pub col: String,
    /// 错误描述
    pub message: String,
}

// ─────────────────────────────────────────────
// 单个 sheet 的导入结果
// ─────────────────────────────────────────────
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SheetImportResult {
    /// Sheet 名称
    pub sheet_name: String,
    /// 是否被识别（如果 Excel 中不存在该 sheet 则为 false）
    pub recognized: bool,
    /// 成功插入行数
    pub inserted: u32,
    /// 解析错误行数（这些行未入库）
    pub error_count: u32,
    /// 错误详情列表（定位到具体单元格）
    pub errors: Vec<CellError>,
}

// ─────────────────────────────────────────────
// 整个导入任务的汇总结果
// ─────────────────────────────────────────────
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ImportSummary {
    /// 批次号（本次导入的唯一 ID）
    pub batch_no: i32,
    /// 每个 sheet 的详细结果
    pub sheets: Vec<SheetImportResult>,
    /// 是否成功提交（false 表示回滚）
    pub committed: bool,
    /// 如果回滚，这里说明原因
    pub rollback_reason: Option<String>,
    /// 导入的 sellers 数量（新增 + 更新）
    pub seller_count: u32,
    /// 发票头成功插入数
    pub header_inserted: u32,
    /// 发票明细成功插入数
    pub detail_inserted: u32,
}

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

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CertifiedHeaderNotFoundItem {
    /// 发票唯一键（inv_unq_key，用于定位缺失的发票号码）
    pub invoice_number: String,
    /// 开票日期
    pub issue_date: Option<String>,
    /// 销售方纳税人识别号
    pub seller_tax_no: Option<String>,
    /// 销售方纳税人名称
    pub seller_name: Option<String>,
}

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
    /// header 未找到的明细列表（发票号码 + 开票日期 + 销售方纳税识别号 + 销售方纳税名称）
    pub header_not_found_keys: Vec<CertifiedHeaderNotFoundItem>,
}
// #[derive(Debug, Clone, Serialize, Deserialize, Type)]
// #[serde(rename_all = "camelCase")]
// pub struct CertifiedImportRequest {
//     pub file_path: String,
// }

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

// ── 追加到 dto.rs ──────────────────────────────────────────

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
