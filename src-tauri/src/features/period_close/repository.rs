// 纳税所属期批次(run)与会计凭证批次(batch)的生命周期。进项税转出与印花税共用，按 module 区分。
//
//   批次状态：待确认(Open) ──确认──▶ 已确认(Confirmed) ──填凭证信息并锁定──▶ 已锁定(Locked)
//                          ◀─取消确认─┘
//   已锁定 ──管理员解锁──▶ 新批次 n+1(Open)，沿用原纳税所属期，旧批次不动。
//
// 规则要点
//   - 只有当前批次是 Open 才能改来源范围、匹配、人工修改。
//   - 确认时把计入的明细写入 *_confirmed_lines；取消确认时删除。已确认/已锁定的明细不再参与别的所属期匹配。
//   - 批次 1 的来源范围可随意改，改了就清空本 run 的匹配结果和人工修改（从头开始）；
//     补充批次(n>1)只能扩大来源范围，不清空任何东西。
//   - 新建纳税所属期：必须晚于本模块已建的最大所属期止；本模块不能同时有两个「批次 1 尚未锁定」的 run。

use rusqlite::{params, OptionalExtension, Row, Transaction};

use super::models::{BatchInfo, BatchState, ConfirmInfo, PeriodStatus};

// ───────────────── 模块表名白名单（表名不来自前端） ─────────────────

pub(crate) struct ModuleTables {
    pub results: &'static str,
    pub overrides: &'static str,
    pub confirmed: &'static str,
    pub label: &'static str,
    /// 来源期间的叫法
    pub source_label: &'static str,
}

pub(crate) fn tables(module: &str) -> Result<ModuleTables, String> {
    match module {
        "input_tax_transfer" => Ok(ModuleTables {
            results: "welfare_transfer_results",
            overrides: "welfare_transfer_overrides",
            confirmed: "welfare_transfer_confirmed_lines",
            label: "进项转出",
            source_label: "认证期间",
        }),
        "stamp_tax" => Ok(ModuleTables {
            results: "stamp_tax_results",
            overrides: "stamp_tax_overrides",
            confirmed: "stamp_tax_confirmed_lines",
            label: "印花税",
            source_label: "凭证期间",
        }),
        _ => Err(format!("未知的模块：{module}")),
    }
}

// ───────────────── 期间校验 ─────────────────

pub(crate) fn validate_period(value: &str) -> Result<String, String> {
    let trimmed = value.trim();
    let month = trimmed.get(4..6).and_then(|m| m.parse::<u32>().ok());
    if trimmed.len() != 6
        || !trimmed.chars().all(|c| c.is_ascii_digit())
        || !month.is_some_and(|m| (1..=12).contains(&m))
    {
        return Err("期间格式错误，应为 6 位数字，例如 202610".to_string());
    }
    Ok(trimmed.to_string())
}

fn month_index(period: &str) -> i32 {
    let y: i32 = period[..4].parse().unwrap_or(0);
    let m: i32 = period[4..6].parse().unwrap_or(1);
    y * 12 + m - 1
}

/// 起 ≤ 止的期间区间，格式合法。
pub(crate) fn validate_range(from: &str, to: &str, what: &str) -> Result<(String, String), String> {
    let (from, to) = (validate_period(from)?, validate_period(to)?);
    if from > to {
        return Err(format!("{what}：结束期间不能早于起始期间"));
    }
    Ok((from, to))
}

/// 纳税所属期：单期，或最多连续 3 个自然月。
pub(crate) fn validate_filing(from: &str, to: &str) -> Result<(String, String), String> {
    let (from, to) = validate_range(from, to, "纳税所属期")?;
    if month_index(&to) - month_index(&from) > 2 {
        return Err("纳税所属期最多跨 3 个自然月".to_string());
    }
    Ok((from, to))
}

pub(crate) fn label(from: &str, to: &str) -> String {
    if from == to {
        from.to_string()
    } else {
        format!("{from}-{to}")
    }
}

/// 期间 YYYYMM 的第一天 YYYY-MM-DD。
pub(crate) fn period_first_day(period: &str) -> String {
    format!("{}-{}-01", &period[..4], &period[4..6])
}

/// 期间 YYYYMM 的最后一天 YYYY-MM-DD。
pub(crate) fn period_last_day(period: &str) -> String {
    let y: i32 = period[..4].parse().unwrap_or(1970);
    let m: u32 = period[4..6].parse().unwrap_or(1);
    let (ny, nm) = if m == 12 { (y + 1, 1) } else { (y, m + 1) };
    chrono::NaiveDate::from_ymd_opt(ny, nm, 1)
        .and_then(|d| d.pred_opt())
        .map(|d| d.format("%Y-%m-%d").to_string())
        .unwrap_or_default()
}

// ───────────────── 读取 ─────────────────

#[derive(Debug, Clone)]
pub(crate) struct RunRow {
    pub id: i64,
    pub module: String,
    pub filing_from: String,
    pub filing_to: String,
    pub src_from: String,
    pub src_to: String,
}

impl RunRow {
    pub fn filing_label(&self) -> String {
        label(&self.filing_from, &self.filing_to)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct BatchRow {
    pub id: i64,
    pub batch_no: u32,
    pub open_reason: Option<String>,
    pub confirmed_by: Option<String>,
    pub confirmed_at: Option<String>,
    pub locked_by: Option<String>,
    pub locked_at: Option<String>,
    pub voucher_no: Option<String>,
    pub posted_date: Option<String>,
    pub preparer: Option<String>,
}

impl BatchRow {
    pub fn state(&self) -> BatchState {
        if self.locked_at.is_some() {
            BatchState::Locked
        } else if self.confirmed_at.is_some() {
            BatchState::Confirmed
        } else {
            BatchState::Open
        }
    }
    fn info(&self) -> BatchInfo {
        BatchInfo {
            batch_no: self.batch_no,
            state: self.state(),
            open_reason: self.open_reason.clone(),
            confirmed_by: self.confirmed_by.clone(),
            confirmed_at: self.confirmed_at.clone(),
            locked_by: self.locked_by.clone(),
            locked_at: self.locked_at.clone(),
            voucher_no: self.voucher_no.clone(),
            posted_date: self.posted_date.clone(),
            preparer: self.preparer.clone(),
        }
    }
}

const RUN_COLS: &str = "id, module, filing_from, filing_to, src_from, src_to";

fn read_run(row: &Row<'_>) -> rusqlite::Result<RunRow> {
    Ok(RunRow {
        id: row.get(0)?,
        module: row.get(1)?,
        filing_from: row.get(2)?,
        filing_to: row.get(3)?,
        src_from: row.get(4)?,
        src_to: row.get(5)?,
    })
}

pub(crate) fn find_run(
    tx: &Transaction<'_>,
    module: &str,
    filing_from: &str,
    filing_to: &str,
) -> Result<Option<RunRow>, String> {
    tx.query_row(
        &format!(
            "SELECT {RUN_COLS} FROM tax_runs
             WHERE module = ?1 AND filing_from = ?2 AND filing_to = ?3"
        ),
        params![module, filing_from, filing_to],
        read_run,
    )
    .optional()
    .map_err(|error| format!("读取纳税所属期批次失败：{error}"))
}

pub(crate) fn load_run(tx: &Transaction<'_>, run_id: i64) -> Result<RunRow, String> {
    tx.query_row(
        &format!("SELECT {RUN_COLS} FROM tax_runs WHERE id = ?1"),
        params![run_id],
        read_run,
    )
    .map_err(|error| format!("读取纳税所属期批次失败：{error}"))
}

/// 区间与 [from, to] 有交集的所属期批次（升序），给查询页列出往期。
pub(crate) fn runs_overlapping(
    tx: &Transaction<'_>,
    module: &str,
    from: &str,
    to: &str,
) -> Result<Vec<RunRow>, String> {
    let mut statement = tx
        .prepare(&format!(
            "SELECT {RUN_COLS} FROM tax_runs
             WHERE module = ?1 AND filing_to >= ?2 AND filing_from <= ?3
             ORDER BY filing_from, filing_to"
        ))
        .map_err(|error| format!("读取往期失败：{error}"))?;
    let rows = statement
        .query_map(params![module, from, to], read_run)
        .map_err(|error| format!("读取往期失败：{error}"))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("读取往期失败：{error}"))
}

pub(crate) fn batches(tx: &Transaction<'_>, run_id: i64) -> Result<Vec<BatchRow>, String> {
    let mut statement = tx
        .prepare(
            "SELECT id, batch_no, open_reason, confirmed_by, confirmed_at, locked_by, locked_at,
                    voucher_no, posted_date, preparer
             FROM tax_run_batches WHERE run_id = ?1 ORDER BY batch_no",
        )
        .map_err(|error| format!("读取批次失败：{error}"))?;
    let rows = statement
        .query_map(params![run_id], |row| {
            Ok(BatchRow {
                id: row.get(0)?,
                batch_no: row.get(1)?,
                open_reason: row.get(2)?,
                confirmed_by: row.get(3)?,
                confirmed_at: row.get(4)?,
                locked_by: row.get(5)?,
                locked_at: row.get(6)?,
                voucher_no: row.get(7)?,
                posted_date: row.get(8)?,
                preparer: row.get(9)?,
            })
        })
        .map_err(|error| format!("读取批次失败：{error}"))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("读取批次失败：{error}"))
}

/// 当前批次 = 批次号最大的一个。
pub(crate) fn current_batch(tx: &Transaction<'_>, run_id: i64) -> Result<BatchRow, String> {
    batches(tx, run_id)?
        .pop()
        .ok_or_else(|| "纳税所属期批次缺少会计凭证批次".to_string())
}

/// 写操作的统一前置检查：run 存在且当前批次是「待确认」。
pub(crate) fn require_open(tx: &Transaction<'_>, run: &RunRow) -> Result<BatchRow, String> {
    let batch = current_batch(tx, run.id)?;
    match batch.state() {
        BatchState::Open => Ok(batch),
        BatchState::Confirmed => Err(format!(
            "纳税所属期 {} 已确认，不能再修改；如需修改请先取消确认",
            run.filing_label()
        )),
        BatchState::Locked => Err(format!(
            "纳税所属期 {} 已锁定，不能再修改；如需补充请联系管理员解锁",
            run.filing_label()
        )),
    }
}

pub(crate) fn status(
    tx: &Transaction<'_>,
    module: &str,
    filing_from: &str,
    filing_to: &str,
    can_unlock: bool,
) -> Result<PeriodStatus, String> {
    tables(module)?;
    let Some(run) = find_run(tx, module, filing_from, filing_to)? else {
        return Ok(PeriodStatus {
            module: module.to_string(),
            filing_from: filing_from.to_string(),
            filing_to: filing_to.to_string(),
            exists: false,
            src_from: None,
            src_to: None,
            state: BatchState::Open,
            batch_no: 1,
            is_supplement: false,
            batches: Vec::new(),
            stored_count: 0,
            can_unlock,
        });
    };
    let all = batches(tx, run.id)?;
    let current = all.last().ok_or("纳税所属期批次缺少会计凭证批次")?;
    Ok(PeriodStatus {
        module: module.to_string(),
        filing_from: run.filing_from.clone(),
        filing_to: run.filing_to.clone(),
        exists: true,
        src_from: Some(run.src_from.clone()),
        src_to: Some(run.src_to.clone()),
        state: current.state(),
        batch_no: current.batch_no,
        is_supplement: current.batch_no > 1,
        batches: all.iter().map(BatchRow::info).collect(),
        stored_count: stored_count(tx, module, run.id)? as u32,
        can_unlock,
    })
}

/// 更改批次 1 的来源期间会清空已有数据时，匹配返回以此开头的错误：`RESET_REQUIRED:{条数}|{说明}`。
/// 前端据此弹出二次确认，用户同意后带 reset = true 重试。
pub(crate) const RESET_REQUIRED: &str = "RESET_REQUIRED:";

// ───────────────── 创建 / 来源范围 ─────────────────

fn count(tx: &Transaction<'_>, table: &str, run_id: i64) -> Result<i64, String> {
    tx.query_row(
        &format!("SELECT COUNT(*) FROM {table} WHERE run_id = ?1"),
        params![run_id],
        |row| row.get(0),
    )
    .map_err(|error| format!("统计失败：{error}"))
}

/// 本 run 已有的匹配结果 + 人工结论条数。
pub(crate) fn stored_count(tx: &Transaction<'_>, module: &str, run_id: i64) -> Result<i64, String> {
    let t = tables(module)?;
    Ok(count(tx, t.results, run_id)? + count(tx, t.overrides, run_id)?)
}

/// 本 run 里尚未确认的匹配结果 + 人工修改条数（补充批次的「新增工作量」，已确认/已锁定的不算）。
pub(crate) fn unconfirmed_count(tx: &Transaction<'_>, module: &str, run_id: i64) -> Result<i64, String> {
    let t = tables(module)?;
    let n = |table: &str| -> Result<i64, String> {
        tx.query_row(
            &format!(
                "SELECT COUNT(*) FROM {table} WHERE run_id = ?1
                   AND detail_id NOT IN (SELECT detail_id FROM {} WHERE run_id = ?1)",
                t.confirmed
            ),
            params![run_id],
            |row| row.get(0),
        )
        .map_err(|error| format!("统计失败：{error}"))
    };
    Ok(n(t.results)? + n(t.overrides)?)
}

/// 清空本 run 的匹配结果（命中规则级联）和人工结论。
fn clear_run_data(tx: &Transaction<'_>, module: &str, run_id: i64) -> Result<(), String> {
    let t = tables(module)?;
    tx.execute(&format!("DELETE FROM {} WHERE run_id = ?1", t.overrides), params![run_id])
        .map_err(|error| format!("清空人工结论失败：{error}"))?;
    tx.execute(&format!("DELETE FROM {} WHERE run_id = ?1", t.results), params![run_id])
        .map_err(|error| format!("清空匹配结果失败：{error}"))?;
    Ok(())
}

/// 匹配前取得（必要时创建）本所属期的 run，并按需要调整来源范围。
///
/// - run 不存在：创建，同时生成批次 1。
/// - run 存在且当前批次待确认：来源范围相同则原样返回；
///   不同 → 批次 1 清空本 run 的结果与人工结论后改范围（已有数据且 `allow_reset` 为假则拒绝）；
///   补充批次只允许扩大范围。
pub(crate) fn prepare_run_for_match(
    tx: &Transaction<'_>,
    module: &str,
    filing: (&str, &str),
    src: (&str, &str),
    allow_reset: bool,
    user: &str,
    now: &str,
) -> Result<RunRow, String> {
    let t = tables(module)?;
    let (filing_from, filing_to) = validate_filing(filing.0, filing.1)?;
    let (src_from, src_to) = validate_range(src.0, src.1, t.source_label)?;
    if src_to > filing_to {
        return Err(format!(
            "{}（{}）不能晚于纳税所属期（{}）",
            t.source_label,
            label(&src_from, &src_to),
            label(&filing_from, &filing_to)
        ));
    }

    if let Some(mut run) = find_run(tx, module, &filing_from, &filing_to)? {
        let batch = require_open(tx, &run)?;
        if run.src_from == src_from && run.src_to == src_to {
            return Ok(run);
        }
        if batch.batch_no == 1 {
            let stored = stored_count(tx, module, run.id)?;
            if stored > 0 && !allow_reset {
                return Err(format!(
                    "{RESET_REQUIRED}{stored}|更改{}将清空本纳税所属期已有的 {stored} 条匹配结果和人工修改，需要确认后重试",
                    t.source_label
                ));
            }
            clear_run_data(tx, module, run.id)?;
        } else if !(src_from <= run.src_from && src_to >= run.src_to) {
            return Err(format!(
                "补充批次只能扩大{}（当前 {}），不能缩小",
                t.source_label,
                label(&run.src_from, &run.src_to)
            ));
        }
        tx.execute(
            "UPDATE tax_runs SET src_from = ?2, src_to = ?3 WHERE id = ?1",
            params![run.id, src_from, src_to],
        )
        .map_err(|error| format!("更新来源期间失败：{error}"))?;
        run.src_from = src_from;
        run.src_to = src_to;
        return Ok(run);
    }

    // 新建：本模块不能同时有两个「批次 1 尚未锁定」的所属期
    let unfinished: Option<(String, String)> = tx
        .query_row(
            "SELECT r.filing_from, r.filing_to FROM tax_runs r
             JOIN tax_run_batches b ON b.run_id = r.id AND b.batch_no = 1
             WHERE r.module = ?1 AND b.locked_at IS NULL
             ORDER BY r.filing_from LIMIT 1",
            params![module],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|error| format!("读取未完成所属期失败：{error}"))?;
    if let Some((from, to)) = unfinished {
        return Err(format!(
            "纳税所属期 {} 还没有锁定，请先完成（确认并锁定）后再建立新的纳税所属期",
            label(&from, &to)
        ));
    }
    let latest: Option<String> = tx
        .query_row(
            "SELECT MAX(filing_to) FROM tax_runs WHERE module = ?1",
            params![module],
            |row| row.get(0),
        )
        .map_err(|error| format!("读取最近纳税所属期失败：{error}"))?;
    if let Some(latest) = latest {
        if filing_from <= latest {
            return Err(format!(
                "新的纳税所属期必须晚于已建立的所属期截至月 {latest}；如需补充更早的所属期，请联系管理员解锁该所属期"
            ));
        }
    }

    tx.execute(
        "INSERT INTO tax_runs (module, filing_from, filing_to, src_from, src_to, created_by, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![module, filing_from, filing_to, src_from, src_to, user, now],
    )
    .map_err(|error| format!("创建纳税所属期失败：{error}"))?;
    let run_id = tx.last_insert_rowid();
    tx.execute(
        "INSERT INTO tax_run_batches (run_id, batch_no, created_by, created_at) VALUES (?1, 1, ?2, ?3)",
        params![run_id, user, now],
    )
    .map_err(|error| format!("创建会计凭证批次失败：{error}"))?;
    load_run(tx, run_id)
}

// ───────────────── 确认 / 取消确认 / 锁定 / 解锁 ─────────────────

/// 模块写完 confirmed_lines 之后调用。
pub(crate) fn mark_confirmed(
    tx: &Transaction<'_>,
    batch_id: i64,
    user: &str,
    now: &str,
) -> Result<(), String> {
    tx.execute(
        "UPDATE tax_run_batches SET confirmed_by = ?2, confirmed_at = ?3 WHERE id = ?1",
        params![batch_id, user, now],
    )
    .map_err(|error| format!("保存确认失败：{error}"))?;
    Ok(())
}

/// 取消确认：只对「已确认、未锁定」的批次。删掉本批次的已确认明细，批次回到待确认；
/// 匹配结果和人工修改原样保留。
pub(crate) fn unconfirm(tx: &Transaction<'_>, run: &RunRow) -> Result<(), String> {
    let t = tables(&run.module)?;
    let batch = current_batch(tx, run.id)?;
    match batch.state() {
        BatchState::Open => return Err("本批次没有确认，无需取消".to_string()),
        BatchState::Locked => return Err("批次已锁定，不能取消确认，请联系管理员解锁".to_string()),
        BatchState::Confirmed => {}
    }
    tx.execute(
        &format!("DELETE FROM {} WHERE batch_id = ?1", t.confirmed),
        params![batch.id],
    )
    .map_err(|error| format!("取消确认失败：{error}"))?;
    tx.execute(
        "UPDATE tax_run_batches SET confirmed_by = NULL, confirmed_at = NULL WHERE id = ?1",
        params![batch.id],
    )
    .map_err(|error| format!("取消确认失败：{error}"))?;
    Ok(())
}

fn non_empty(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

/// 锁定：填写本批进项转出 / 印花税的会计凭证信息。本批有明细时三项必填。
pub(crate) fn lock(
    tx: &Transaction<'_>,
    run: &RunRow,
    voucher_no: &str,
    posted_date: &str,
    preparer: &str,
    user: &str,
    now: &str,
) -> Result<(), String> {
    let t = tables(&run.module)?;
    let batch = current_batch(tx, run.id)?;
    match batch.state() {
        BatchState::Open => return Err("请先确认，再锁定".to_string()),
        BatchState::Locked => return Err("本批次已经锁定".to_string()),
        BatchState::Confirmed => {}
    }
    let lines: i64 = tx
        .query_row(
            &format!("SELECT COUNT(*) FROM {} WHERE batch_id = ?1", t.confirmed),
            params![batch.id],
            |row| row.get(0),
        )
        .map_err(|error| format!("统计本批明细失败：{error}"))?;

    let (voucher_no, posted_date, preparer) =
        (non_empty(voucher_no), non_empty(posted_date), non_empty(preparer));
    if lines > 0 && (voucher_no.is_none() || posted_date.is_none() || preparer.is_none()) {
        return Err(format!(
            "本批有 {lines} 行明细，必须填写{}会计凭证号、入账日期、做账人",
            t.label
        ));
    }
    if let Some(date) = &posted_date {
        chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d")
            .map_err(|_| format!("入账日期格式应为 YYYY-MM-DD：{date}"))?;
    }
    tx.execute(
        "UPDATE tax_run_batches
         SET locked_by = ?2, locked_at = ?3, voucher_no = ?4, posted_date = ?5, preparer = ?6
         WHERE id = ?1",
        params![batch.id, user, now, voucher_no, posted_date, preparer],
    )
    .map_err(|error| format!("锁定失败：{error}"))?;
    Ok(())
}

/// 管理员解锁：当前批次必须已锁定；新开批次 n+1（待确认），沿用原纳税所属期，旧批次不动。
/// 管理员身份由调用方在命令层校验。
pub(crate) fn unlock(
    tx: &Transaction<'_>,
    run: &RunRow,
    reason: &str,
    user: &str,
    now: &str,
) -> Result<(), String> {
    let reason = non_empty(reason).ok_or("请填写解锁原因（如：税务局核查补税）")?;
    let batch = current_batch(tx, run.id)?;
    if batch.state() != BatchState::Locked {
        return Err("只有已锁定的纳税所属期才能解锁".to_string());
    }
    tx.execute(
        "INSERT INTO tax_run_batches (run_id, batch_no, open_reason, created_by, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![run.id, batch.batch_no + 1, reason, user, now],
    )
    .map_err(|error| format!("解锁失败：{error}"))?;
    Ok(())
}

/// 管理员重新锁定：补充批次(n>1)还没确认、且没有新增明细时，撤销这个空的补充批次。
pub(crate) fn relock(tx: &Transaction<'_>, run: &RunRow, pending: i64) -> Result<(), String> {
    let batch = current_batch(tx, run.id)?;
    if batch.batch_no == 1 || batch.state() != BatchState::Open {
        return Err("只有已解锁、尚未确认的补充批次才能重新锁定".to_string());
    }
    if pending > 0 {
        return Err(format!(
            "补充批次已有 {pending} 条新增明细，请确认并锁定，而不是直接撤销"
        ));
    }
    tx.execute("DELETE FROM tax_run_batches WHERE id = ?1", params![batch.id])
        .map_err(|error| format!("重新锁定失败：{error}"))?;
    Ok(())
}

// ───────────────── 已确认明细信息（复核表 / 查询页共用） ─────────────────

/// 与 LEFT JOIN 配套：`LEFT JOIN {confirmed} cf ON cf.detail_id = d.id
///  LEFT JOIN tax_run_batches b ON b.id = cf.batch_id LEFT JOIN tax_runs tr ON tr.id = cf.run_id`
pub(crate) const CONFIRM_COLS: &str = "cf.run_id AS cf_run_id, tr.filing_from AS cf_from, \
     tr.filing_to AS cf_to, b.batch_no AS cf_batch_no, b.confirmed_by AS cf_by, \
     b.confirmed_at AS cf_at, b.locked_at AS cf_locked_at, b.voucher_no AS cf_voucher_no, \
     b.posted_date AS cf_posted_date, b.preparer AS cf_preparer";

#[derive(Debug, Clone)]
pub(crate) struct RawConfirm {
    pub run_id: i64,
    pub filing_from: String,
    pub filing_to: String,
    pub batch_no: u32,
    pub confirmed_by: Option<String>,
    pub confirmed_at: Option<String>,
    pub locked_at: Option<String>,
    pub voucher_no: Option<String>,
    pub posted_date: Option<String>,
    pub preparer: Option<String>,
}

pub(crate) fn read_confirm(row: &Row<'_>) -> rusqlite::Result<Option<RawConfirm>> {
    let Some(run_id) = row.get::<_, Option<i64>>("cf_run_id")? else {
        return Ok(None);
    };
    Ok(Some(RawConfirm {
        run_id,
        filing_from: row.get("cf_from")?,
        filing_to: row.get("cf_to")?,
        batch_no: row.get("cf_batch_no")?,
        confirmed_by: row.get("cf_by")?,
        confirmed_at: row.get("cf_at")?,
        locked_at: row.get("cf_locked_at")?,
        voucher_no: row.get("cf_voucher_no")?,
        posted_date: row.get("cf_posted_date")?,
        preparer: row.get("cf_preparer")?,
    }))
}

pub(crate) fn confirm_info(raw: &RawConfirm, current_run_id: i64) -> ConfirmInfo {
    ConfirmInfo {
        same_run: raw.run_id == current_run_id,
        filing_period: label(&raw.filing_from, &raw.filing_to),
        batch_no: raw.batch_no,
        state: if raw.locked_at.is_some() { BatchState::Locked } else { BatchState::Confirmed },
        confirmed_by: raw.confirmed_by.clone(),
        confirmed_at: raw.confirmed_at.clone(),
        locked_at: raw.locked_at.clone(),
        voucher_no: raw.voucher_no.clone(),
        posted_date: raw.posted_date.clone(),
        preparer: raw.preparer.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    const M: &str = "input_tax_transfer";

    /// 只建这个模块用到的表，避免依赖完整 schema 和分词扩展。
    fn setup() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE tax_runs (
                 id INTEGER PRIMARY KEY, module TEXT NOT NULL, filing_from TEXT NOT NULL, filing_to TEXT NOT NULL,
                 src_from TEXT NOT NULL, src_to TEXT NOT NULL, created_by TEXT NOT NULL, created_at TEXT NOT NULL,
                 UNIQUE (module, filing_from, filing_to));
             CREATE TABLE tax_run_batches (
                 id INTEGER PRIMARY KEY, run_id INTEGER NOT NULL, batch_no INTEGER NOT NULL, open_reason TEXT,
                 created_by TEXT NOT NULL, created_at TEXT NOT NULL, confirmed_by TEXT, confirmed_at TEXT,
                 locked_by TEXT, locked_at TEXT, voucher_no TEXT, posted_date TEXT, preparer TEXT,
                 UNIQUE (run_id, batch_no));
             CREATE TABLE welfare_transfer_results (id INTEGER PRIMARY KEY, run_id INTEGER, detail_id INTEGER);
             CREATE TABLE welfare_transfer_overrides (id INTEGER PRIMARY KEY, run_id INTEGER, detail_id INTEGER);
             CREATE TABLE welfare_transfer_confirmed_lines (id INTEGER PRIMARY KEY, run_id INTEGER, batch_id INTEGER, detail_id INTEGER);
             CREATE TABLE stamp_tax_results (id INTEGER PRIMARY KEY, run_id INTEGER, detail_id INTEGER);
             CREATE TABLE stamp_tax_overrides (id INTEGER PRIMARY KEY, run_id INTEGER, detail_id INTEGER);
             CREATE TABLE stamp_tax_confirmed_lines (id INTEGER PRIMARY KEY, run_id INTEGER, batch_id INTEGER, detail_id INTEGER);",
        )
        .unwrap();
        conn
    }

    fn prepare(
        tx: &Transaction<'_>,
        module: &str,
        filing: (&str, &str),
        src: (&str, &str),
        allow_reset: bool,
    ) -> Result<RunRow, String> {
        prepare_run_for_match(tx, module, filing, src, allow_reset, "u", "t")
    }

    /// 把批次 1 走完：确认 + 锁定
    fn finish(tx: &Transaction<'_>, run: &RunRow) {
        let b = current_batch(tx, run.id).unwrap();
        mark_confirmed(tx, b.id, "u", "t").unwrap();
        lock(tx, run, "", "", "", "u", "t").unwrap();
    }

    #[test]
    fn filing_range_validation() {
        assert!(validate_filing("202601", "202603").is_ok());
        assert!(validate_filing("202601", "202604").is_err()); // 超过 3 个月
        assert!(validate_filing("202603", "202601").is_err());
        assert!(validate_filing("2026-01", "202601").is_err());
        assert!(validate_filing("202511", "202601").is_ok()); // 跨年 3 个月
    }

    #[test]
    fn create_run_checks_source_and_order_and_unfinished() {
        let mut conn = setup();
        let tx = conn.transaction().unwrap();
        // 来源期间不能晚于纳税所属期
        assert!(prepare(&tx, M, ("202601", "202603"), ("202601", "202604"), false).is_err());
        let r1 = prepare(&tx, M, ("202601", "202603"), ("202512", "202603"), false).unwrap();
        assert_eq!(current_batch(&tx, r1.id).unwrap().batch_no, 1);
        // 批次 1 未锁定：不能建新的所属期；别的模块不受影响
        assert!(prepare(&tx, M, ("202607", "202607"), ("202607", "202607"), false).unwrap_err().contains("还没有锁定"));
        prepare(&tx, "stamp_tax", ("202607", "202607"), ("202607", "202607"), false).unwrap();
        finish(&tx, &r1);
        // 必须晚于已建的最大所属期止；中间月份可以跳过
        assert!(prepare(&tx, M, ("202603", "202603"), ("202603", "202603"), false).is_err());
        prepare(&tx, M, ("202607", "202609"), ("202607", "202609"), false).unwrap();
    }

    #[test]
    fn changing_source_on_batch1_clears_data_after_confirmation_of_reset() {
        let mut conn = setup();
        let tx = conn.transaction().unwrap();
        let run = prepare(&tx, M, ("202601", "202603"), ("202601", "202603"), false).unwrap();
        tx.execute("INSERT INTO welfare_transfer_results (run_id, detail_id) VALUES (?1, 1)", [run.id]).unwrap();
        tx.execute("INSERT INTO welfare_transfer_overrides (run_id, detail_id) VALUES (?1, 2)", [run.id]).unwrap();
        // 范围相同：原样返回
        prepare(&tx, M, ("202601", "202603"), ("202601", "202603"), false).unwrap();
        // 范围变了：有数据且没确认重置 → 拒绝，数据不动
        assert!(prepare(&tx, M, ("202601", "202603"), ("202512", "202603"), false).unwrap_err().contains("清空"));
        assert_eq!(stored_count(&tx, M, run.id).unwrap(), 2);
        // 确认重置：清空并改范围
        let run = prepare(&tx, M, ("202601", "202603"), ("202512", "202603"), true).unwrap();
        assert_eq!(run.src_from, "202512");
        assert_eq!(stored_count(&tx, M, run.id).unwrap(), 0);
    }

    #[test]
    fn confirm_unconfirm_lock_flow_and_state_guards() {
        let mut conn = setup();
        let tx = conn.transaction().unwrap();
        let run = prepare(&tx, M, ("202601", "202601"), ("202601", "202601"), false).unwrap();
        let b = current_batch(&tx, run.id).unwrap();
        assert!(lock(&tx, &run, "", "", "", "u", "t").is_err()); // 未确认不能锁定
        assert!(unconfirm(&tx, &run).is_err()); // 未确认无需取消

        mark_confirmed(&tx, b.id, "u", "t").unwrap();
        assert!(require_open(&tx, &run).unwrap_err().contains("取消确认")); // 已确认只读
        assert!(prepare(&tx, M, ("202601", "202601"), ("202601", "202601"), false).is_err());
        // 取消确认回到待确认，删掉本批已确认明细
        tx.execute("INSERT INTO welfare_transfer_confirmed_lines (run_id, batch_id, detail_id) VALUES (?1, ?2, 1)", params![run.id, b.id]).unwrap();
        unconfirm(&tx, &run).unwrap();
        assert!(require_open(&tx, &run).is_ok());
        let n: i64 = tx.query_row("SELECT COUNT(*) FROM welfare_transfer_confirmed_lines", [], |r| r.get(0)).unwrap();
        assert_eq!(n, 0);

        // 有明细时锁定必须填凭证信息
        mark_confirmed(&tx, b.id, "u", "t").unwrap();
        tx.execute("INSERT INTO welfare_transfer_confirmed_lines (run_id, batch_id, detail_id) VALUES (?1, ?2, 1)", params![run.id, b.id]).unwrap();
        assert!(lock(&tx, &run, "", "", "", "u", "t").is_err());
        assert!(lock(&tx, &run, "V1", "2026-13-40", "张三", "u", "t").is_err());
        lock(&tx, &run, "V1", "2026-02-05", "张三", "u", "t").unwrap();
        assert_eq!(status(&tx, M, "202601", "202601", false).unwrap().state, BatchState::Locked);
        assert!(unconfirm(&tx, &run).is_err()); // 已锁定不能取消
    }

    #[test]
    fn unlock_opens_supplement_batch_and_allows_only_expanding_source() {
        let mut conn = setup();
        let tx = conn.transaction().unwrap();
        let run = prepare(&tx, M, ("202501", "202503"), ("202501", "202503"), false).unwrap();
        assert!(unlock(&tx, &run, "原因", "admin", "t").is_err()); // 未锁定不能解锁
        finish(&tx, &run);
        assert!(unlock(&tx, &run, "  ", "admin", "t").is_err()); // 原因必填
        unlock(&tx, &run, "税务局核查补税", "admin", "t").unwrap();

        let s = status(&tx, M, "202501", "202503", true).unwrap();
        assert_eq!((s.batch_no, s.state, s.is_supplement), (2, BatchState::Open, true));
        assert_eq!(s.batches.len(), 2);
        assert_eq!(s.batches[0].state, BatchState::Locked); // 批次 1 保持锁定

        // 另有更晚的所属期正在进行时，解锁历史所属期不受限制（上面已解锁），也不影响新建判断
        tx.execute("INSERT INTO welfare_transfer_results (run_id, detail_id) VALUES (?1, 1)", [run.id]).unwrap();
        // 补充批次：缩小被拒绝；扩大允许且不清空数据
        assert!(prepare(&tx, M, ("202501", "202503"), ("202502", "202503"), true).is_err());
        let run = prepare(&tx, M, ("202501", "202503"), ("202412", "202503"), false).unwrap();
        assert_eq!(run.src_from, "202412");
        assert_eq!(stored_count(&tx, M, run.id).unwrap(), 1);

        // 空的补充批次可以重新锁定（撤销）；有新增明细则拒绝
        assert!(relock(&tx, &run, 3).is_err());
        relock(&tx, &run, 0).unwrap();
        assert_eq!(status(&tx, M, "202501", "202503", true).unwrap().state, BatchState::Locked);
        assert!(relock(&tx, &run, 0).is_err()); // 已经是锁定状态
    }

    #[test]
    fn modules_are_independent_and_table_names_whitelisted() {
        let mut conn = setup();
        let tx = conn.transaction().unwrap();
        prepare(&tx, M, ("202601", "202601"), ("202601", "202601"), false).unwrap();
        assert!(!status(&tx, "stamp_tax", "202601", "202601", false).unwrap().exists);
        assert!(tables("evil; DROP TABLE x").is_err());
        assert_eq!(label("202601", "202603"), "202601-202603");
        assert_eq!(period_first_day("202602"), "2026-02-01");
        assert_eq!(period_last_day("202602"), "2026-02-28");
        assert_eq!(period_last_day("202612"), "2026-12-31");
    }
}
