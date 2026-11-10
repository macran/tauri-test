use serde::{Deserialize, Serialize};

/// 纳税所属期批次(run)的标识。前端只传模块 + 所属期，run 在第一次匹配时才创建。
#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RunKey {
    /// "input_tax_transfer" | "stamp_tax"
    pub module: String,
    /// 纳税所属期起，6 位数字，例如 "202601"
    pub filing_from: String,
    /// 纳税所属期止；单期时与 filing_from 相同，最多连续 3 个自然月
    pub filing_to: String,
}

/// 只指明纳税所属期（模块由命令本身决定）。
#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FilingRequest {
    pub filing_from: String,
    pub filing_to: String,
}

/// 确认 / 取消确认 / 重新锁定的请求：与 RunKey 相同。
pub type PeriodActionRequest = RunKey;

/// 锁定时填写的本批进项转出 / 印花税会计凭证信息。
#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LockRequest {
    pub module: String,
    pub filing_from: String,
    pub filing_to: String,
    /// 进项转出 / 印花税的会计凭证号（不是 AP/IC/银行/差旅的原记账凭证）
    pub voucher_no: String,
    /// 入账日期 YYYY-MM-DD
    pub posted_date: String,
    /// 做账人
    pub preparer: String,
}

#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct UnlockRequest {
    pub module: String,
    pub filing_from: String,
    pub filing_to: String,
    /// 解锁原因（如：税务局核查补税），必填
    pub reason: String,
}

/// 批次状态：待确认 → 已确认 → 已锁定。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum BatchState {
    /// 可改来源范围、匹配、人工修改
    Open,
    /// 只读，可导出清单，可取消确认
    Confirmed,
    /// 永久只读；凭证信息已填
    Locked,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct BatchInfo {
    pub batch_no: u32,
    pub state: BatchState,
    /// 补充批次的解锁原因
    pub open_reason: Option<String>,
    pub confirmed_by: Option<String>,
    pub confirmed_at: Option<String>,
    pub locked_by: Option<String>,
    pub locked_at: Option<String>,
    pub voucher_no: Option<String>,
    pub posted_date: Option<String>,
    pub preparer: Option<String>,
}

/// 一个纳税所属期批次当前的完整状态，供状态栏使用。
/// `exists = false` 表示还没点过「匹配」，run 尚未创建。
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PeriodStatus {
    pub module: String,
    pub filing_from: String,
    pub filing_to: String,
    pub exists: bool,
    /// 来源期间范围（进项转出 = 认证期间，印花税 = 凭证期间）
    pub src_from: Option<String>,
    pub src_to: Option<String>,
    /// 当前（编号最大）批次的状态；run 不存在时为 Open
    pub state: BatchState,
    /// 当前批次号；run 不存在时为 1
    pub batch_no: u32,
    /// 当前批次是补充批次（批次号 > 1）
    pub is_supplement: bool,
    pub batches: Vec<BatchInfo>,
    /// 本纳税所属期已有的匹配结果 + 人工修改条数；改批次 1 的来源范围会清空它们
    pub stored_count: u32,
    /// 前端用它决定是否显示「解锁」；真正的权限在命令里实时校验。
    pub can_unlock: bool,
}

/// 一条已确认明细所属的确认信息。复核表、查询页、导出都用它显示批次与凭证信息。
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ConfirmInfo {
    /// true = 在当前纳税所属期批次内确认；false = 在另一个纳税所属期确认（当前批次只显示、不计入合计）
    pub same_run: bool,
    /// 确认它的纳税所属期，如 "202601" 或 "202601-202603"
    pub filing_period: String,
    pub batch_no: u32,
    /// Confirmed 或 Locked
    pub state: BatchState,
    pub confirmed_by: Option<String>,
    pub confirmed_at: Option<String>,
    pub locked_at: Option<String>,
    /// 进项转出 / 印花税自己的会计凭证信息（锁定后才有）
    pub voucher_no: Option<String>,
    pub posted_date: Option<String>,
    pub preparer: Option<String>,
}
