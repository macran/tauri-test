use serde::{Deserialize, Serialize};

// ───────────────────────── 请求 ─────────────────────────

#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PeriodRangeRequest {
    /// 起始期间，6 位数字，例如 "202601"
    pub period_from: String,
    /// 结束期间；单期时与 period_from 相同
    pub period_to: String,
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
    pub period_from: String,
    pub period_to: String,
    #[serde(flatten)]
    pub patch: OverridePatch,
}

#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SaveOverridesBatchRequest {
    pub period_from: String,
    pub period_to: String,
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

    pub tax_period: String,
    /// 认证清单回填的凭证号；无凭证号时为 None
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
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ReviewSummary {
    pub period_from: String,
    pub period_to: String,

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
    /// 最终转出条数
    pub included_row_count: u32,

    /// 最终转出税额合计（分）
    pub total_transfer_tax: String,
    pub rows: Vec<ReviewRow>,
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
    pub tax_period: String,
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
}