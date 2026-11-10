use serde::{Deserialize, Serialize};

use crate::features::period_close::{BatchState, ConfirmInfo, RawConfirm};

// ───────────────────────── 请求 ─────────────────────────

/// 匹配：纳税所属期 + 凭证期间（来源范围，不晚于纳税所属期止）。
/// 第一次匹配时创建该纳税所属期的批次；批次 1 更改凭证期间会清空本所属期已有的结果和人工修改，
/// 此时需要 reset = true（前端二次确认后传入）。
#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct StampTaxMatchRequest {
    pub filing_from: String,
    pub filing_to: String,
    /// 凭证入账期间 YYYYMM
    pub src_from: String,
    pub src_to: String,
    #[serde(default)]
    pub reset: bool,
}

/// 一条人工修改。两个字段均为 None 表示清除该明细的人工结论。
#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct StampTaxOverridePatch {
    pub detail_id: u32,
    /// None = 清除人工判定，回到系统匹配结果
    pub included: Option<bool>,
    /// 人工指定的印花税类别；指定后视为计入，不计入时会被忽略
    pub stamp_tax_category_code: Option<String>,
    pub remark: Option<String>,
}

#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct StampTaxSaveOverrideRequest {
    pub filing_from: String,
    pub filing_to: String,
    #[serde(flatten)]
    pub patch: StampTaxOverridePatch,
}

#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct StampTaxSaveOverridesBatchRequest {
    pub filing_from: String,
    pub filing_to: String,
    pub items: Vec<StampTaxOverridePatch>,
}

// ───────────────────────── 响应 ─────────────────────────

/// 规则的一个匹配条件，前端逐条渲染。
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct StampTaxRuleCondition {
    /// 维度名，如 "销售方税号"
    pub field: String,
    /// 匹配方式，如 "等于" / "不等于"
    pub operator: String,
    /// 配置值，逗号分隔的原始字符串
    pub values: String,
}

/// 一条命中的规则，含条件明细、印花税类别与规则备注。
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct StampTaxMatchedRule {
    pub rule_id: u32,
    pub rule_category: String,
    /// 规则对应的印花税税目名称
    pub stamp_tax_category: Option<String>,
    /// 规则备注，供用户判断
    pub note: String,
    /// 该规则要求人工核对
    pub needs_manual_review: bool,
    /// 规则当前是否仍启用；禁用后历史命中仍可见
    pub enabled: bool,
    pub conditions: Vec<StampTaxRuleCondition>,
}

/// 系统匹配事实。展示出来的明细全部已做账（在 4 张凭证表之一找到凭证号），
/// 因此只区分命中与否，不再有「缺凭证号」状态——未做账的明细从一开始就不会被选入核对列表。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum StampTaxMatchState {
    /// 参与匹配并命中规则
    Matched,
    /// 参与匹配但未命中任何规则
    NotMatched,
}

/// 当前印花税计入结论的来源。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum StampTaxDecisionSource {
    /// 机器按规则匹配得出
    System,
    /// 人工修改过，重新匹配不会覆盖
    Manual,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct StampTaxReviewRow {
    pub detail_id: u32,
    pub header_id: u32,
    pub line_no: u32,

    /// 凭证入账期间 YYYYMM（凭证的入账/过账日期所在月份）
    pub booked_period: String,
    /// 原记账凭证号（AP/Bank/IC 凭证号，或差旅认证清单凭证号）；不是印花税自己的会计凭证号
    pub voucher_no: String,

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
    /// 计税基数（分）：专用发票为金额，其余为价税合计
    pub stamp_base: String,
    /// 印花税额（分）：基数 × 税率；未选定类别（无税率）时为 0
    pub stamp_tax_amount: String,

    /// 系统匹配事实
    pub match_state: StampTaxMatchState,
    /// 结论来源：机器匹配 / 人工修改
    pub decision_source: StampTaxDecisionSource,

    /// 命中规则中存在需人工核对的，供筛选
    pub needs_manual_review: bool,
    /// 计入判定被人工设定过
    pub decision_manually_set: bool,

    /// 最终是否计入印花税计算
    pub final_included: bool,

    /// 最终采用的印花税类别：人工指定优先，否则取唯一命中的规则
    pub stamp_tax_category_code: Option<String>,
    pub stamp_tax_category: Option<String>,
    /// 税率（百万分率），实际税率 = rate_ppm / 1000000
    pub rate_ppm: Option<u32>,
    /// 类别是人工指定的
    pub category_manually_set: bool,
    /// 计入印花税但税率未确定（命中多条规则，或手工计入未命中的明细），用户必须选定类别
    pub needs_category_choice: bool,

    pub remark: Option<String>,
    /// 系统本次命中的规则；未命中时为空数组
    pub matched_rules: Vec<StampTaxMatchedRule>,

    /// 已确认或已锁定时的批次与印花税凭证信息；未确认为 None
    pub confirm: Option<ConfirmInfo>,
    /// 已在**其他**纳税所属期确认：本行只显示、锁定，不计入本所属期的任何合计
    pub confirmed_elsewhere: bool,
    /// 不能再修改（已确认、已锁定，或确认在别的所属期）
    pub locked: bool,
    /// 计入本纳税所属期的印花税（最终计入，且不是别处已确认）
    pub counted: bool,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct StampTaxReviewSummary {
    pub filing_from: String,
    pub filing_to: String,
    /// 该纳税所属期批次是否已创建（点过「匹配」）
    pub exists: bool,
    /// 凭证期间（来源范围）
    pub src_from: Option<String>,
    pub src_to: Option<String>,
    /// 当前批次状态；非「待确认」时前端只读
    pub state: BatchState,
    pub batch_no: u32,

    pub total_row_count: u32,
    /// 系统命中条数，不受人工影响
    pub matched_row_count: u32,
    /// 参与匹配但未命中
    pub not_matched_count: u32,
    /// 命中规则要求人工核对的条数
    pub needs_review_count: u32,
    /// 人工修改过的条数
    pub manual_count: u32,
    /// 最终计入印花税计算的条数
    pub included_row_count: u32,
    /// 尚须指定印花税类别的条数
    pub unresolved_count: u32,
    /// 本所属期行级印花税之和（分，仅参考；采集表按子目汇总后计算）
    pub total_stamp_tax: String,
    /// 其中当前批次的条数与行级税额；补充批次只含新增
    pub batch_row_count: u32,
    pub batch_stamp_tax: String,

    pub rows: Vec<StampTaxReviewRow>,
}

/// 查询页的一个纳税所属期：批次状态 + 该期的明细。
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct StampHistoryPeriod {
    pub review: StampTaxReviewSummary,
}

/// 查询范围：仅已确认/已锁定，还是全部（含待确认的计入）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum StampHistoryView {
    Confirmed,
    All,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct StampTaxMatchRunResult {
    /// 本次已做账、参与匹配的明细条数
    pub qualified_item_count: u32,
    /// 系统命中条数
    pub matched_item_count: u32,
    /// 因存在人工结论而不受本次匹配影响的条数
    pub preserved_override_count: u32,
    pub review: StampTaxReviewSummary,
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

#[allow(dead_code)] // rate_ppm 由类别表在显示时查，这里的副本暂未读取
#[derive(Debug, Clone)]
pub(crate) struct RuleForMatch {
    pub id: i64,
    pub rule_category: String,
    pub stamp_tax_category_code: Option<String>,
    pub stamp_tax_category: Option<String>,
    pub rate_ppm: Option<i64>,
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

/// 增值税专用发票（含数电专票、数电纸质专票、增值税电子专用发票）的 inv_type：2、3、7、13。
/// 与应付凭证导入校验「增票」用的是同一组取值。
const SPECIAL_INV_TYPES: [i64; 4] = [2, 3, 7, 13];

/// 印花税计税基数：专用发票用金额（不含税），其余票种用价税合计。单位：分。
pub(crate) fn stamp_base(inv_type: i64, amount: i64, total: i64) -> i64 {
    if SPECIAL_INV_TYPES.contains(&inv_type) {
        amount
    } else {
        total
    }
}

/// 印花税额（分）= |基数| × ppm / 1000000，四舍五入到分，符号随基数。
pub(crate) fn stamp_amount(base: i64, rate_ppm: i64) -> i64 {
    (base.abs() * rate_ppm + 500_000) / 1_000_000 * base.signum()
}

#[cfg(test)]
mod stamp_base_tests {
    use super::*;

    #[test]
    fn special_invoices_use_amount_others_use_total() {
        // 2 数电专票、3 增值税专用发票、7 数电纸质专票：用金额
        for t in [2, 3, 7, 13] {
            assert_eq!(stamp_base(t, 10_000, 11_300), 10_000);
        }
        // 其余票种（普票、数电普票、电子普票等）：用价税合计
        for t in [1, 4, 5, 6, 8, 9, 10] {
            assert_eq!(stamp_base(t, 10_000, 11_300), 11_300);
        }
    }

    #[test]
    fn amount_rounds_to_cent_and_follows_sign() {
        assert_eq!(stamp_amount(10_000, 300), 3); // 100 元 × 0.3‰ = 0.03 元
        assert_eq!(stamp_amount(-10_000, 300), -3); // 红字发票，符号随基数
        assert_eq!(stamp_amount(0, 300), 0);
    }
}

/// 核对列表主干查询出来的原始行，尚未派生最终状态。
#[derive(Debug, Clone)]
pub(crate) struct RawReviewRow {
    pub detail_id: i64,
    pub header_id: i64,
    pub line_no: i64,
    pub booked_period: String,
    pub voucher_no: String,
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
    pub inv_type: i64,
    pub result_id: Option<i64>,
    pub override_included: Option<i64>,
    pub override_category_code: Option<String>,
    pub override_remark: Option<String>,
    /// 这条明细的已确认快照（可能在别的纳税所属期）；没确认过为 None
    pub confirm: Option<RawConfirm>,
    pub confirmed_category: Option<String>,
    pub confirmed_base: Option<i64>,
    pub confirmed_rate: Option<i64>,
    pub confirmed_tax: Option<i64>,
    pub confirmed_decision: Option<String>,
}
