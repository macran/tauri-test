//! 进项税转出 / 印花税 共用流程层：纳税所属期结果集(run) → 会计凭证批次(batch) → 确认锁定 → 管理员解锁。
//!
//! 状态机
//!   open ──确认批次1──▶ confirmed ──管理员解锁──▶ reopened ──确认批次n──▶ confirmed
//!                                                  └──管理员重新锁定(无新增)──▶ confirmed
//!
//! 规则
//!   - open / reopened 可匹配、可人工修改；confirmed 拒绝一切写操作
//!   - 已确认明细(confirmed_lines) 永不修改，reopened 状态下只能“新增”转出
//!   - 确认前必须先导出本批清单；确认时比对导出指纹，导出后又改过结果则拒绝
//!   - 首次匹配前先选纳税所属期（跨期最多3个月；新所属期必须晚于上次已建立的所属期）；补充批次沿用原所属期

use anyhow::{anyhow, bail, Context, Result};
use chrono::{Duration, Local, NaiveDate};
use rusqlite::{params, Connection, OptionalExtension, Params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use specta::Type;

// ─────────────────────────────────────────────────────────────
// 基础类型
// ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum TaxModule {
    InputTaxTransfer,
    StampTax,
}

impl TaxModule {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InputTaxTransfer => "input_tax_transfer",
            Self::StampTax => "stamp_tax",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::InputTaxTransfer => "进项税转出",
            Self::StampTax => "印花税",
        }
    }
    pub fn from_db(s: &str) -> Result<Self> {
        match s {
            "input_tax_transfer" => Ok(Self::InputTaxTransfer),
            "stamp_tax" => Ok(Self::StampTax),
            o => bail!("未知模块：{o}"),
        }
    }
}

/// 期间区间，YYYYMM，from == to 即单期
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PeriodRange {
    pub from: String,
    pub to: String,
}

impl PeriodRange {
    pub fn validate(&self, what: &str) -> Result<()> {
        validate_period(&self.from).with_context(|| format!("{what}（起）"))?;
        validate_period(&self.to).with_context(|| format!("{what}（止）"))?;
        if self.from > self.to {
            bail!("{what}：起始期间 {} 晚于结束期间 {}", self.from, self.to);
        }
        Ok(())
    }
    pub fn overlaps(&self, from: &str, to: &str) -> bool {
        !(self.to.as_str() < from || self.from.as_str() > to)
    }
    pub fn label(&self) -> String {
        if self.from == self.to {
            self.from.clone()
        } else {
            format!("{}-{}", self.from, self.to)
        }
    }
    /// 起始月第一天 YYYY-MM-DD
    pub fn first_day(&self) -> String {
        format!("{}-{}-01", &self.from[..4], &self.from[4..6])
    }
    /// 结束月最后一天 YYYY-MM-DD
    pub fn last_day(&self) -> String {
        let y: i32 = self.to[..4].parse().unwrap_or(1970);
        let m: u32 = self.to[4..6].parse().unwrap_or(1);
        let next = if m == 12 {
            NaiveDate::from_ymd_opt(y + 1, 1, 1)
        } else {
            NaiveDate::from_ymd_opt(y, m + 1, 1)
        };
        next.map(|d| (d - Duration::days(1)).format("%Y-%m-%d").to_string())
            .unwrap_or_default()
    }
}

pub fn validate_period(p: &str) -> Result<()> {
    if p.len() != 6 || !p.bytes().all(|b| b.is_ascii_digit()) {
        bail!("期间格式应为 YYYYMM：{p}");
    }
    let m: u32 = p[4..].parse()?;
    if !(1..=12).contains(&m) {
        bail!("期间月份无效：{p}");
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Open,
    Confirmed,
    Reopened,
}

impl RunStatus {
    fn from_db(s: &str) -> Result<Self> {
        match s {
            "open" => Ok(Self::Open),
            "confirmed" => Ok(Self::Confirmed),
            "reopened" => Ok(Self::Reopened),
            o => bail!("未知状态：{o}"),
        }
    }
}

/// 匹配类型（列表展示 & 快照 decision 字段）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum MatchType {
    /// 机器命中，未经人工
    AutoHit,
    /// 机器未命中，未经人工
    NoHit,
    /// 人工复核过，结论与机器一致
    Reviewed,
    /// 人工修改了机器结论（含：改税额 / 改印花税类别 / 未命中改为转出 / 命中改为不转出）
    Manual,
}

impl MatchType {
    pub fn decision_str(self) -> &'static str {
        match self {
            Self::AutoHit | Self::NoHit => "auto",
            Self::Reviewed => "reviewed",
            Self::Manual => "manual",
        }
    }
    pub fn from_decision(s: &str) -> Self {
        match s {
            "manual" => Self::Manual,
            "reviewed" => Self::Reviewed,
            _ => Self::AutoHit,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::AutoHit => "机器匹配",
            Self::NoHit => "未命中",
            Self::Reviewed => "机器匹配(已复核)",
            Self::Manual => "人工修改",
        }
    }
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RunBatch {
    pub id: i64,
    pub batch_no: i64,
    pub created_at: String,
    pub exported_at: Option<String>,
    pub exported_by: Option<String>,
    pub export_line_count: Option<i64>,
    pub export_tax_total: Option<i64>,
    pub acct_voucher_no: Option<String>,
    pub acct_posted_date: Option<String>,
    pub acct_preparer: Option<String>,
    pub confirmed_by: Option<String>,
    pub confirmed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TaxRun {
    pub id: i64,
    pub module: TaxModule,
    /// 旧数据兼容字段：最后/首个来源期间；业务以 src_periods 为准
    pub src_period: PeriodRange,
    /// 本结果集累计选择的认证/凭证期间
    pub src_periods: Vec<PeriodRange>,
    /// 纳税所属期：创建批次时先选定
    pub filing_period: Option<PeriodRange>,
    pub status: RunStatus,
    pub created_by: String,
    pub created_at: String,
    pub last_matched_at: Option<String>,
    pub last_modified_at: Option<String>,
    pub batches: Vec<RunBatch>,
}

impl TaxRun {
    pub fn pending_batch(&self) -> Option<&RunBatch> {
        self.batches.iter().find(|b| b.confirmed_at.is_none())
    }
    pub fn is_editable(&self) -> bool {
        matches!(self.status, RunStatus::Open | RunStatus::Reopened)
    }
    pub fn ensure_editable(&self) -> Result<()> {
        if !self.is_editable() {
            bail!(
                "纳税所属期结果集 {} 已确认锁定（纳税所属期 {}），如需修改请联系管理员解锁",
                self.filing_period.as_ref().map(|p| p.label()).unwrap_or_else(|| self.src_period.label()),
                self.filing_period.as_ref().map(|p| p.label()).unwrap_or_default()
            );
        }
        Ok(())
    }
    pub fn ensure_module(&self, m: TaxModule) -> Result<()> {
        if self.module != m {
            bail!("纳税所属期结果集 {} 不属于{}", self.id, m.label());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RunEvent {
    pub id: i64,
    pub event: String,
    pub detail: String,
    pub actor: String,
    pub at: String,
}

/// 每次列表查询时返回的统计，前端用于按钮可用性与提示
#[derive(Debug, Clone, Default, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LineStats {
    pub total_lines: i64,
    pub auto_hit_count: i64,
    pub included_count: i64,
    pub included_tax: i64,
    /// 本批待确认（转出且未确认）
    pub pending_count: i64,
    pub pending_tax: i64,
    /// 待复核（命中需人工核对规则 / 类别冲突，且未人工判定、未确认）
    pub needs_review_count: i64,
    /// 转出但缺少有效印花税类别（仅印花税）
    pub invalid_count: i64,
    pub manual_count: i64,
    pub confirmed_count: i64,
    pub confirmed_tax: i64,
    /// 本批已导出过
    pub exported: bool,
    /// 导出后结果又有变化（需重新导出才能确认）
    pub export_stale: bool,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct MatchSummary {
    pub run: TaxRun,
    pub rule_count: i64,
    pub candidate_count: i64,
    pub hit_count: i64,
    pub skipped_manual: i64,
    pub skipped_confirmed: i64,
    pub skipped_other_run: i64,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ApplyResult {
    pub applied: i64,
    pub skipped_locked: i64,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ExportResult {
    pub path: String,
    pub line_count: i64,
    pub tax_total: i64,
}

#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ConfirmInput {
    pub run_id: i64,
    pub acct_voucher_no: Option<String>,
    pub acct_posted_date: Option<String>,
    pub acct_preparer: Option<String>,
}

/// 确认前由模块计算的「当前待确认内容」
pub struct PendingSnapshot {
    pub fingerprint: Vec<u8>,
    pub line_count: i64,
    pub tax_total: i64,
}

pub struct AcctVoucher {
    pub voucher_no: Option<String>,
    pub posted_date: Option<String>,
    pub preparer: Option<String>,
}

// ─────────────────────────────────────────────────────────────
// 工具
// ─────────────────────────────────────────────────────────────

pub fn now() -> String {
    Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

pub fn fen_to_yuan(v: i64) -> f64 {
    v as f64 / 100.0
}

/// 内容指纹：对每行的规范化字符串排序后 sha256
pub fn fingerprint(items: impl IntoIterator<Item = String>) -> Vec<u8> {
    let mut v: Vec<String> = items.into_iter().collect();
    v.sort();
    let mut h = Sha256::new();
    for s in &v {
        h.update(s.as_bytes());
        h.update(b"\n");
    }
    h.finalize().to_vec()
}

fn non_empty(s: &Option<String>) -> Option<String> {
    s.as_ref().map(|x| x.trim().to_string()).filter(|x| !x.is_empty())
}

pub fn is_admin(conn: &Connection, user: &str) -> Result<bool> {
    Ok(conn
        .prepare_cached("SELECT 1 FROM app_admins WHERE username = ?1")?
        .exists([user])?)
}

pub fn ensure_admin(conn: &Connection, user: &str) -> Result<()> {
    if !is_admin(conn, user)? {
        bail!("仅管理员可执行该操作（当前用户：{user}）");
    }
    Ok(())
}

pub fn log_event(conn: &Connection, run_id: i64, event: &str, detail: &str, actor: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO tax_run_events (run_id, event, detail, actor, at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![run_id, event, detail, actor, now()],
    )?;
    Ok(())
}

pub fn touch_run(conn: &Connection, run_id: i64, matched: bool) -> Result<()> {
    let ts = now();
    if matched {
        conn.execute(
            "UPDATE tax_runs SET last_matched_at = ?2, last_modified_at = ?2 WHERE id = ?1",
            params![run_id, ts],
        )?;
    } else {
        conn.execute("UPDATE tax_runs SET last_modified_at = ?2 WHERE id = ?1", params![run_id, ts])?;
    }
    Ok(())
}

// ─────────────────────────────────────────────────────────────
// 读取
// ─────────────────────────────────────────────────────────────

fn load_sources(conn: &Connection, run_id: i64) -> Result<Vec<PeriodRange>> {
    let mut stmt = conn.prepare_cached(
        "SELECT src_period_from, src_period_to FROM tax_run_sources WHERE run_id = ?1 ORDER BY src_period_from, src_period_to",
    )?;
    let rows = stmt
        .query_map([run_id], |r| Ok(PeriodRange { from: r.get(0)?, to: r.get(1)? }))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

fn query_runs<P: Params>(conn: &Connection, where_sql: &str, p: P) -> Result<Vec<TaxRun>> {
    let sql = format!(
        "SELECT id, module, src_period_from, src_period_to, filing_period_from, filing_period_to,
                status, created_by, created_at, last_matched_at, last_modified_at
         FROM tax_runs {where_sql}
         ORDER BY filing_period_from DESC, filing_period_to DESC, id DESC"
    );
    let mut stmt = conn.prepare(&sql)?;
    let raw = stmt
        .query_map(p, |r| {
            Ok((
                r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?,
                r.get::<_, Option<String>>(4)?, r.get::<_, Option<String>>(5)?, r.get::<_, String>(6)?,
                r.get::<_, String>(7)?, r.get::<_, String>(8)?, r.get::<_, Option<String>>(9)?,
                r.get::<_, Option<String>>(10)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let mut out = Vec::with_capacity(raw.len());
    for (id, module, sf, st, ff, ft, status, cb, ca, lm, lmod) in raw {
        let sources = load_sources(conn, id)?;
        let src_period = sources.last().cloned().unwrap_or(PeriodRange { from: sf, to: st });
        out.push(TaxRun {
            id,
            module: TaxModule::from_db(&module)?,
            src_period,
            src_periods: sources,
            filing_period: match (ff, ft) {
                (Some(from), Some(to)) => Some(PeriodRange { from, to }),
                _ => None,
            },
            status: RunStatus::from_db(&status)?,
            created_by: cb,
            created_at: ca,
            last_matched_at: lm,
            last_modified_at: lmod,
            batches: load_batches(conn, id)?,
        });
    }
    Ok(out)
}

fn load_batches(conn: &Connection, run_id: i64) -> Result<Vec<RunBatch>> {
    let mut stmt = conn.prepare_cached(
        "SELECT id, batch_no, created_at, exported_at, exported_by, export_line_count, export_tax_total,
                acct_voucher_no, acct_posted_date, acct_preparer, confirmed_by, confirmed_at
         FROM tax_run_batches WHERE run_id = ?1 ORDER BY batch_no",
    )?;
    let rows = stmt.query_map([run_id], |r| {
        Ok(RunBatch {
            id: r.get(0)?, batch_no: r.get(1)?, created_at: r.get(2)?, exported_at: r.get(3)?,
            exported_by: r.get(4)?, export_line_count: r.get(5)?, export_tax_total: r.get(6)?,
            acct_voucher_no: r.get(7)?, acct_posted_date: r.get(8)?, acct_preparer: r.get(9)?,
            confirmed_by: r.get(10)?, confirmed_at: r.get(11)?,
        })
    })?.collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn list_runs(conn: &Connection, module: TaxModule) -> Result<Vec<TaxRun>> {
    query_runs(conn, "WHERE module = ?1", [module.as_str()])
}

pub fn load_run(conn: &Connection, run_id: i64) -> Result<TaxRun> {
    query_runs(conn, "WHERE id = ?1", [run_id])?
        .into_iter().next().ok_or_else(|| anyhow!("纳税所属期批次 {run_id} 不存在"))
}

pub fn find_run(conn: &Connection, module: TaxModule, filing: &PeriodRange) -> Result<Option<TaxRun>> {
    filing.validate("纳税所属期")?;
    query_runs(
        conn,
        "WHERE module = ?1 AND filing_period_from = ?2 AND filing_period_to = ?3",
        params![module.as_str(), filing.from, filing.to],
    ).map(|v| v.into_iter().next())
}

pub fn list_events(conn: &Connection, run_id: i64) -> Result<Vec<RunEvent>> {
    let mut stmt = conn.prepare(
        "SELECT id, event, detail, actor, at FROM tax_run_events WHERE run_id = ?1 ORDER BY id DESC",
    )?;
    let rows = stmt
        .query_map([run_id], |r| {
            Ok(RunEvent { id: r.get(0)?, event: r.get(1)?, detail: r.get(2)?, actor: r.get(3)?, at: r.get(4)? })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// 导出指纹（用于 export_stale 判定）
pub fn batch_export_fingerprint(conn: &Connection, batch_id: i64) -> Result<Option<Vec<u8>>> {
    Ok(conn
        .query_row(
            "SELECT export_fingerprint FROM tax_run_batches WHERE id = ?1",
            [batch_id],
            |r| r.get::<_, Option<Vec<u8>>>(0),
        )
        .optional()?
        .flatten())
}

// ─────────────────────────────────────────────────────────────
// 写入
// ─────────────────────────────────────────────────────────────

/// 所属期是流程主键；来源期间作为该所属期下可追加的筛选范围。
pub fn get_or_create_run(
    conn: &Connection,
    module: TaxModule,
    filing: &PeriodRange,
    source: &PeriodRange,
    user: &str,
) -> Result<TaxRun> {
    filing.validate("纳税所属期")?;
    source.validate(if module == TaxModule::InputTaxTransfer { "认证期间" } else { "凭证期间" })?;
    if month_index(&filing.to) - month_index(&filing.from) > 2 {
        bail!("纳税所属期最多跨3个自然月");
    }
    if let Some(run) = find_run(conn, module, filing)? {
        if run.status == RunStatus::Confirmed {
            bail!("纳税所属期 {} 已确认锁定；如需补充或修改，请由管理员解锁该所属期", filing.label());
        }
        let overlap: Option<(String, String)> = conn.query_row(
            "SELECT src_period_from, src_period_to FROM tax_run_sources
             WHERE run_id = ?1 AND src_period_from <= ?3 AND src_period_to >= ?2
               AND NOT (src_period_from = ?2 AND src_period_to = ?3) LIMIT 1",
            params![run.id, source.from, source.to],
            |r| Ok((r.get(0)?, r.get(1)?)),
        ).optional()?;
        if let Some((f, t)) = overlap {
            bail!("所选来源期间 {} 与本所属期已加入的 {} 重叠；请调整范围，避免重复列示明细",
                source.label(), PeriodRange { from: f, to: t }.label());
        }
        conn.execute(
            "INSERT OR IGNORE INTO tax_run_sources (run_id, src_period_from, src_period_to, added_by, added_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![run.id, source.from, source.to, user, now()],
        )?;
        return load_run(conn, run.id);
    }

    let active: Option<(Option<String>, Option<String>)> = conn.query_row(
        "SELECT filing_period_from, filing_period_to FROM tax_runs
         WHERE module = ?1 AND status IN ('open','reopened') ORDER BY id DESC LIMIT 1",
        [module.as_str()], |r| Ok((r.get(0)?, r.get(1)?)),
    ).optional()?;
    if let Some((f, t)) = active {
        match (f, t) {
            (Some(from), Some(to)) => bail!("纳税所属期 {} 尚未完成，请先完成或由管理员处理该批次", PeriodRange { from, to }.label()),
            _ => bail!("存在未迁移完成的旧批次，请先处理该旧批次后再新建纳税所属期"),
        }
    }
    let latest: Option<String> = conn.query_row(
        "SELECT MAX(filing_period_to) FROM tax_runs WHERE module = ?1 AND filing_period_to IS NOT NULL",
        [module.as_str()], |r| r.get(0),
    )?;
    if let Some(prev) = latest {
        if filing.from <= prev {
            bail!("新的纳税所属期必须晚于此前已建立的所属期截至月 {prev}；如需更正，请解锁原所属期");
        }
    }
    let ts = now();
    conn.execute(
        "INSERT INTO tax_runs (module, src_period_from, src_period_to, filing_period_from, filing_period_to,
                               status, created_by, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, 'open', ?6, ?7)",
        params![module.as_str(), source.from, source.to, filing.from, filing.to, user, ts],
    )?;
    let id = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO tax_run_sources (run_id, src_period_from, src_period_to, added_by, added_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id, source.from, source.to, user, ts],
    )?;
    conn.execute("INSERT INTO tax_run_batches (run_id, batch_no, created_at) VALUES (?1, 1, ?2)", params![id, ts])?;
    log_event(conn, id, "created", &format!("纳税所属期{}；来源期间{}", filing.label(), source.label()), user)?;
    load_run(conn, id)
}

fn month_index(p: &str) -> i32 {
    p[..4].parse::<i32>().unwrap_or(0) * 12 + p[4..6].parse::<i32>().unwrap_or(1) - 1
}

pub fn record_export(
    conn: &Connection,
    run: &TaxRun,
    snapshot: &PendingSnapshot,
    path: &str,
    user: &str,
) -> Result<()> {
    let batch = run.pending_batch().ok_or_else(|| anyhow!("没有待确认的批次"))?;
    conn.execute(
        "UPDATE tax_run_batches
         SET exported_at = ?2, exported_by = ?3, export_fingerprint = ?4,
             export_line_count = ?5, export_tax_total = ?6
         WHERE id = ?1",
        params![batch.id, now(), user, snapshot.fingerprint, snapshot.line_count, snapshot.tax_total],
    )?;
    log_event(
        conn,
        run.id,
        "exported",
        &format!("批次{} 行数{} 税额(分){} → {}", batch.batch_no, snapshot.line_count, snapshot.tax_total, path),
        user,
    )
}

/// 确认前的公共校验。返回待确认批次与规范化后的会计凭证信息。
pub fn begin_confirm(
    conn: &Connection,
    module: TaxModule,
    input: &ConfirmInput,
    current: &PendingSnapshot,
) -> Result<(TaxRun, RunBatch, AcctVoucher)> {
    let run = load_run(conn, input.run_id)?;
    run.ensure_module(module)?;
    run.ensure_editable()?;
    let batch = run.pending_batch().cloned().ok_or_else(|| anyhow!("没有待确认的批次"))?;

    match batch_export_fingerprint(conn, batch.id)? {
        None => bail!("请先导出本批{}清单交会计做账，再回填凭证信息确认", module.label()),
        Some(fp) if fp != current.fingerprint => {
            bail!("导出清单后结果又有修改，请重新导出、交会计核对后再确认")
        }
        _ => {}
    }
    if batch.batch_no > 1 && current.line_count == 0 {
        bail!("补充批次没有新增明细，无需确认；如需恢复锁定，请由管理员执行“重新锁定”");
    }

    let voucher = AcctVoucher {
        voucher_no: non_empty(&input.acct_voucher_no),
        posted_date: non_empty(&input.acct_posted_date),
        preparer: non_empty(&input.acct_preparer),
    };
    if current.line_count > 0
        && (voucher.voucher_no.is_none() || voucher.posted_date.is_none() || voucher.preparer.is_none())
    {
        bail!("本批有 {} 行明细，必须填写会计凭证号、入账日期、做账人", current.line_count);
    }
    if let Some(d) = &voucher.posted_date {
        NaiveDate::parse_from_str(d, "%Y-%m-%d").with_context(|| format!("入账日期格式应为 YYYY-MM-DD：{d}"))?;
    }

    if run.filing_period.is_none() {
        bail!("该批次缺少纳税所属期；请联系管理员检查旧数据迁移状态");
    }
    Ok((run, batch, voucher))
}

/// 模块写完 confirmed_lines 后调用
pub fn finish_confirm(
    conn: &Connection,
    run: &TaxRun,
    batch: &RunBatch,
    voucher: &AcctVoucher,
    snapshot: &PendingSnapshot,
    confirmed_at: &str,
    user: &str,
) -> Result<()> {
    conn.execute(
        "UPDATE tax_run_batches
         SET acct_voucher_no = ?2, acct_posted_date = ?3, acct_preparer = ?4,
             confirmed_by = ?5, confirmed_at = ?6
         WHERE id = ?1",
        params![batch.id, voucher.voucher_no, voucher.posted_date, voucher.preparer, user, confirmed_at],
    )?;
    let filing = run.filing_period.as_ref().ok_or_else(|| anyhow!("纳税所属期缺失"))?;
    conn.execute(
        "UPDATE tax_runs SET status = 'confirmed', last_modified_at = ?2 WHERE id = ?1",
        params![run.id, confirmed_at],
    )?;
    log_event(
        conn,
        run.id,
        "confirmed",
        &format!(
            "批次{} 纳税所属期{} 凭证{} 行数{} 税额(分){}",
            batch.batch_no,
            filing.label(),
            voucher.voucher_no.clone().unwrap_or_else(|| "-".into()),
            snapshot.line_count,
            snapshot.tax_total
        ),
        user,
    )
}

/// 管理员解锁：confirmed → reopened，并开一个新的待确认批次
pub fn unlock_run(conn: &Connection, run_id: i64, reason: &str, user: &str) -> Result<TaxRun> {
    ensure_admin(conn, user)?;
    if reason.trim().is_empty() {
        bail!("请填写解锁原因（如：税务局核查补税）");
    }
    let run = load_run(conn, run_id)?;
    if run.status != RunStatus::Confirmed {
        bail!("只有已确认的纳税所属期结果集才能解锁");
    }
    let next_no = run.batches.iter().map(|b| b.batch_no).max().unwrap_or(0) + 1;
    let ts = now();
    conn.execute(
        "UPDATE tax_runs SET status = 'reopened', last_modified_at = ?2 WHERE id = ?1",
        params![run_id, ts],
    )?;
    conn.execute(
        "INSERT INTO tax_run_batches (run_id, batch_no, created_at) VALUES (?1, ?2, ?3)",
        params![run_id, next_no, ts],
    )?;
    log_event(conn, run_id, "unlocked", &format!("开启补充批次{next_no}：{}", reason.trim()), user)?;
    load_run(conn, run_id)
}

/// 管理员重新锁定：reopened 且没有新增转出明细时，撤销补充批次
pub fn relock_run(conn: &Connection, run_id: i64, reason: &str, user: &str, pending_count: i64) -> Result<TaxRun> {
    ensure_admin(conn, user)?;
    let run = load_run(conn, run_id)?;
    if run.status != RunStatus::Reopened {
        bail!("只有已解锁的纳税所属期结果集才能重新锁定");
    }
    if pending_count > 0 {
        bail!("已有 {pending_count} 条新增明细，请导出补充清单交会计做账后确认，而不是直接锁定");
    }
    conn.execute("DELETE FROM tax_run_batches WHERE run_id = ?1 AND confirmed_at IS NULL", [run_id])?;
    conn.execute(
        "UPDATE tax_runs SET status = 'confirmed', last_modified_at = ?2 WHERE id = ?1",
        params![run_id, now()],
    )?;
    log_event(conn, run_id, "relocked", reason.trim(), user)?;
    load_run(conn, run_id)
}

/// 删除从未确认过的所属期结果集（选错期间时用）。cleanup 负责删除模块自己的结果/人工表。
pub fn delete_open_run(conn: &Connection, run_id: i64, cleanup: impl FnOnce(&Connection) -> Result<()>) -> Result<()> {
    let run = load_run(conn, run_id)?;
    if run.status != RunStatus::Open || run.batches.iter().any(|b| b.confirmed_at.is_some()) {
        bail!("该纳税所属期结果集已有确认记录，不能删除");
    }
    cleanup(conn)?;
    conn.execute("DELETE FROM tax_runs WHERE id = ?1", [run_id])?;
    Ok(())
}
