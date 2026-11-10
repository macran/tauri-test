use serde::{Deserialize, Serialize};

use crate::features::period_close::{BatchState, ConfirmInfo, RawConfirm};

// ───────────────────────── 请求 ─────────────────────────

/// 匹配：纳税所属期 + 认证期间（来源范围）。
/// 第一次匹配时创建该纳税所属期的批次；批次 1 更改认证期间会清空本所属期已有的结果和人工修改，
/// 此时需要 reset = true（前端二次确认后传入）。
#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MatchRequest {
    /// 纳税所属期起，6 位数字，例如 "202601"
    pub filing_from: String,
    /// 纳税所属期止；单期时与 filing_from 相同，最多连续 3 个自然月
    pub filing_to: String,
    /// 认证期间起（不晚于纳税所属期止）
    pub src_from: String,
    pub src_to: String,
    #[serde(default)]
    pub reset: bool,
}

/// 一条人工修改。每个字段为 None 表示清除该维度的人工结论。
#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct OverridePatch {
    pub detail_id: u32,
    /// None = 清除人工判定，回到系统匹配结果
    pub included: Option<bool>,
    /// 单位：分，字符串。None = 清除人工税额，回到派生默认值
    pub transfer_tax: Option<String>,
    pub remark: Option<String>,
}

#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SaveOverrideRequest {
    pub filing_from: String,
    pub filing_to: String,
    #[serde(flatten)]
    pub patch: OverridePatch,
}

#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SaveOverridesBatchRequest {
    pub filing_from: String,
    pub filing_to: String,
    pub items: Vec<OverridePatch>,
}

// ───────────────────────── 响应 ─────────────────────────

/// 规则的一个匹配条件，前端逐条渲染。
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RuleCondition {
    /// 维度名，如 "销售方税号"
    pub field: String,
    /// 匹配方式，如 "等于" / "不等于"
    pub operator: String,
    /// 配置值，逗号分隔的原始字符串
    pub values: String,
}

/// 一条命中的规则，含条件明细与规则备注。
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MatchedRule {
    pub rule_id: u32,
    pub rule_category: String,
    /// 规则备注，供用户判断
    pub note: String,
    /// 该规则要求人工核对
    pub needs_manual_review: bool,
    /// 规则当前是否仍启用；禁用后历史命中仍可见
    pub enabled: bool,
    pub conditions: Vec<RuleCondition>,
}

/// 系统匹配事实。只取决于凭证号与规则，人工修改不会改变它。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum MatchState {
    /// 认证清单无凭证号，未参与匹配
    MissingVoucher,
    /// 参与匹配并命中规则
    Matched,
    /// 参与匹配但未命中任何规则
    NotMatched,
}

/// 当前转出结论的来源。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum DecisionSource {
    /// 机器按规则匹配得出
    System,
    /// 人工修改过，重新匹配不会覆盖
    Manual,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ReviewRow {
    pub detail_id: u32,
    pub header_id: u32,
    pub line_no: u32,

    /// 认证期间 YYYYMM
    pub cert_period: String,
    /// 认证清单回填的原记账凭证号（AP/IC/银行/差旅）；无凭证号时为 None
    pub voucher_no: Option<String>,

    pub invoice_no: String,
    pub issue_date: String,
    pub seller_tax_no: String,
    pub seller_name: String,

    pub goods_name: String,
    pub tax_code: Option<String>,
    pub goods_category: Option<String>,
    pub tax_rate: Option<String>,

    // 金额单位：分，字符串传输，避免 i32 溢出与 Specta BigInt
    pub amount: String,
    pub tax: String,
    pub total: String,

    /// 系统匹配事实
    pub match_state: MatchState,
    /// 结论来源：机器匹配 / 人工修改
    pub decision_source: DecisionSource,

    /// 命中规则中存在需人工核对的，供筛选
    pub needs_manual_review: bool,
    /// 转出判定被人工设定过
    pub decision_manually_set: bool,
    /// 转出税额被人工改过
    pub tax_manually_edited: bool,

    /// 最终是否转出
    pub final_included: bool,
    /// 最终转出税额（分）
    pub final_transfer_tax: String,

    pub remark: Option<String>,
    /// 系统本次命中的规则；未命中时为空数组
    pub matched_rules: Vec<MatchedRule>,

    /// 已确认或已锁定时的批次与进项转出凭证信息；未确认为 None
    pub confirm: Option<ConfirmInfo>,
    /// 已在**其他**纳税所属期确认：本行只显示、锁定，不计入本所属期的任何合计
    pub confirmed_elsewhere: bool,
    /// 不能再修改（已确认、已锁定，或确认在别的所属期）
    pub locked: bool,
    /// 计入本纳税所属期的转出（最终转出，且不是别处已确认）
    pub counted: bool,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ReviewSummary {
    pub filing_from: String,
    pub filing_to: String,
    /// 该纳税所属期批次是否已创建（点过「匹配」）
    pub exists: bool,
    /// 认证期间（来源范围）
    pub src_from: Option<String>,
    pub src_to: Option<String>,
    /// 当前批次状态；非「待确认」时前端只读
    pub state: BatchState,
    pub batch_no: u32,

    pub total_row_count: u32,
    pub missing_voucher_count: u32,
    /// 系统命中条数，不受人工影响
    pub matched_row_count: u32,
    /// 参与匹配但未命中
    pub not_matched_count: u32,
    /// 命中规则要求人工核对的条数
    pub needs_review_count: u32,
    /// 人工修改过的条数
    pub manual_count: u32,
    /// 本所属期最终转出条数（不含别处已确认）
    pub included_row_count: u32,

    /// 本所属期最终转出税额合计（分）
    pub total_transfer_tax: String,
    /// 其中当前批次的条数与税额；补充批次只含新增，已锁定批次不重复计入
    pub batch_row_count: u32,
    pub batch_transfer_tax: String,
    pub rows: Vec<ReviewRow>,
}

/// 查询页的一个纳税所属期：批次状态 + 该期的明细。
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TransferHistoryPeriod {
    pub review: ReviewSummary,
}

/// 查询范围：仅已确认/已锁定，还是全部（含待确认的转出）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum HistoryView {
    /// 只看已确认（含已锁定）的转出明细
    Confirmed,
    /// 本所属期所有已入库的结果（命中 + 人工修改，不含未命中），含待确认
    All,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MatchRunResult {
    /// 本次回填到 certified_list 的凭证号条数
    pub voucher_filled_count: u32,
    /// 有凭证号、参与匹配的明细条数
    pub qualified_item_count: u32,
    /// 系统命中条数
    pub matched_item_count: u32,
    /// 因存在人工结论而不受本次匹配影响的条数
    pub preserved_override_count: u32,
    pub review: ReviewSummary,
}

// ───────────────────────── 内部结构 ─────────────────────────

#[derive(Debug, Clone)]
pub(crate) struct MatchCandidate {
    pub detail_id: i64,
    pub header_id: i64,
    pub seller_tax_no: String,
    pub tax_code: Option<String>,
    pub goods_category: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct RuleForMatch {
    pub id: i64,
    pub rule_category: String,
    pub note: String,
    pub needs_manual_review: bool,
    pub enabled: bool,

    pub seller_tax_id_match_type: Option<String>,
    pub seller_tax_id_values: Option<String>,
    pub tax_code_match_type: Option<String>,
    pub tax_code_values: Option<String>,
    pub tax_name_match_type: Option<String>,
    pub tax_name_values: Option<String>,
}

/// 核对列表主干查询出来的原始行，尚未派生最终状态。
#[derive(Debug, Clone)]
pub(crate) struct RawReviewRow {
    pub detail_id: i64,
    pub header_id: i64,
    pub line_no: i64,
    pub cert_period: String,
    pub voucher_no: Option<String>,
    pub invoice_no: String,
    pub issue_date: String,
    pub seller_tax_no: String,
    pub seller_name: String,
    pub goods_name: String,
    pub tax_code: Option<String>,
    pub goods_category: Option<String>,
    pub tax_rate: Option<String>,
    pub amount: i64,
    pub tax: i64,
    pub total: i64,
    pub result_id: Option<i64>,
    pub override_included: Option<i64>,
    pub override_transfer_tax: Option<i64>,
    pub override_remark: Option<String>,
    /// 这条明细的已确认快照（可能在别的纳税所属期）；没确认过为 None
    pub confirm: Option<RawConfirm>,
    /// 快照里的转出税额（分）与决定来源，和 confirm 同时存在
    pub confirmed_tax: Option<i64>,
    pub confirmed_decision: Option<String>,
}