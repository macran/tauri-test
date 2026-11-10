use std::collections::{HashMap, HashSet};

use chrono::Local;
use rusqlite::Transaction;
use tauri::State;

use crate::features::period_close::repository::{self as period_repo, RunRow};
use crate::features::period_close::{
    BatchState, FilingRequest, PeriodActionRequest, PeriodStatus,
};
use crate::{AppRuntimeState, DbState};

use super::match_engine::{self, MatchInput};
use super::models::{
    DecisionSource, HistoryView, MatchRequest, MatchRunResult, MatchState, MatchedRule,
    OverridePatch, RawReviewRow, ReviewRow, ReviewSummary, RuleForMatch, SaveOverrideRequest,
    SaveOverridesBatchRequest, TransferHistoryPeriod,
};
use super::repository::{self, ConfirmLine};

const MODULE: &str = "input_tax_transfer";

// ───────────────── 校验与转换 ─────────────────

fn parse_fen(value: &str) -> Result<i64, String> {
    let trimmed = value.trim();
    trimmed
        .parse::<i64>()
        .map_err(|_| format!("转出税额格式错误：{trimmed}"))
}

fn to_u32(value: i64, label: &str) -> Result<u32, String> {
    u32::try_from(value).map_err(|_| format!("{label}超出可导出范围"))
}

fn now() -> String {
    Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

// ───────────────── 派生核对行 ─────────────────

fn derive_review_row(
    raw: &RawReviewRow,
    current_run_id: i64,
    rule_hits: &HashMap<i64, Vec<i64>>,
    rules_by_id: &HashMap<i64, &RuleForMatch>,
) -> Result<ReviewRow, String> {
    let system_matched = raw.result_id.is_some();

    // 命中规则不受人工影响：只要系统本次命中，条件与备注就一直可见。
    let matched_rules: Vec<MatchedRule> = raw
        .result_id
        .and_then(|result_id| rule_hits.get(&result_id))
        .map(|rule_ids| {
            rule_ids
                .iter()
                .filter_map(|rule_id| rules_by_id.get(rule_id))
                .map(|rule| match_engine::to_matched_rule(rule))
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?
        .unwrap_or_default();

    let needs_manual_review = matched_rules.iter().any(|rule| rule.needs_manual_review);

    let override_included = raw.override_included.map(|value| value != 0);
    let decision_manually_set = raw.override_included.is_some();
    let tax_manually_edited = raw.override_transfer_tax.is_some();
    let manual_overridden =
        decision_manually_set || tax_manually_edited || raw.override_remark.is_some();

    // 系统事实：只看凭证号与命中情况。
    let match_state = if raw.voucher_no.is_none() {
        MatchState::MissingVoucher
    } else if system_matched {
        MatchState::Matched
    } else {
        MatchState::NotMatched
    };

    let confirm = raw
        .confirm
        .as_ref()
        .map(|c| period_repo::confirm_info(c, current_run_id));
    let confirmed_elsewhere = confirm.as_ref().is_some_and(|c| !c.same_run);

    // 已确认的明细以确认时的快照为准（金额、决定来源），不再随规则或人工变化。
    let (decision_source, final_included, final_transfer_tax) =
        if let (Some(_), Some(tax)) = (&raw.confirm, raw.confirmed_tax) {
            let source = match raw.confirmed_decision.as_deref() {
                Some("manual") => DecisionSource::Manual,
                _ => DecisionSource::System,
            };
            (source, true, tax)
        } else {
            // 结论来源：人工动过任一维度即为人工结论，重新匹配不会覆盖。
            let source = if manual_overridden { DecisionSource::Manual } else { DecisionSource::System };
            // 最终结论：人工判定优先，未表态则沿用系统命中结果。
            let included = override_included.unwrap_or(system_matched);
            // 最终税额：人工税额优先；否则转出取明细税额，不转出取 0。
            let tax = raw
                .override_transfer_tax
                .unwrap_or(if included { raw.tax } else { 0 });
            (source, included, tax)
        };

    Ok(ReviewRow {
        detail_id: to_u32(raw.detail_id, "发票明细 ID")?,
        header_id: to_u32(raw.header_id, "发票头 ID")?,
        line_no: to_u32(raw.line_no, "明细行号")?,
        cert_period: raw.cert_period.clone(),
        voucher_no: raw.voucher_no.clone(),
        invoice_no: raw.invoice_no.clone(),
        issue_date: raw.issue_date.clone(),
        seller_tax_no: raw.seller_tax_no.clone(),
        seller_name: raw.seller_name.clone(),
        goods_name: raw.goods_name.clone(),
        tax_code: raw.tax_code.clone(),
        goods_category: raw.goods_category.clone(),
        tax_rate: raw.tax_rate.clone(),
        amount: raw.amount.to_string(),
        tax: raw.tax.to_string(),
        total: raw.total.to_string(),
        match_state,
        decision_source,
        needs_manual_review,
        decision_manually_set,
        tax_manually_edited,
        final_included,
        final_transfer_tax: final_transfer_tax.to_string(),
        remark: raw.override_remark.clone(),
        matched_rules,
        locked: confirm.is_some(),
        counted: final_included && !confirmed_elsewhere,
        confirmed_elsewhere,
        confirm,
    })
}

fn empty_summary(filing: (&str, &str)) -> ReviewSummary {
    ReviewSummary {
        filing_from: filing.0.to_string(),
        filing_to: filing.1.to_string(),
        exists: false,
        src_from: None,
        src_to: None,
        state: BatchState::Open,
        batch_no: 1,
        total_row_count: 0,
        missing_voucher_count: 0,
        matched_row_count: 0,
        not_matched_count: 0,
        needs_review_count: 0,
        manual_count: 0,
        included_row_count: 0,
        total_transfer_tax: "0".to_string(),
        batch_row_count: 0,
        batch_transfer_tax: "0".to_string(),
        rows: Vec::new(),
    }
}

/// 某纳税所属期的核对列表与合计。run 不存在时返回空汇总。
/// `view = Confirmed` 只列本所属期已确认（含已锁定）的明细。
fn build_review_summary(
    tx: &Transaction<'_>,
    filing: (&str, &str),
    view: HistoryView,
) -> Result<ReviewSummary, String> {
    let Some(run) = period_repo::find_run(tx, MODULE, filing.0, filing.1)? else {
        return Ok(empty_summary(filing));
    };
    summarize_run(tx, &run, view)
}

fn summarize_run(
    tx: &Transaction<'_>,
    run: &RunRow,
    view: HistoryView,
) -> Result<ReviewSummary, String> {
    let current = period_repo::current_batch(tx, run.id)?;
    let raw_rows = repository::fetch_review_rows(
        tx,
        run.id,
        &run.src_from,
        &run.src_to,
        view == HistoryView::Confirmed,
    )?;

    let result_ids: Vec<i64> = raw_rows.iter().filter_map(|row| row.result_id).collect();
    let rule_hits = repository::fetch_rule_hits(tx, &result_ids)?;

    // 禁用的规则也读出来，历史命中仍需展示条件与备注。
    let all_rules = repository::fetch_rules(tx, false)?;
    let rules_by_id: HashMap<i64, &RuleForMatch> =
        all_rules.iter().map(|rule| (rule.id, rule)).collect();

    let mut rows = Vec::with_capacity(raw_rows.len());
    let mut s = empty_summary(filing_of(run));
    let (mut total_tax, mut batch_tax) = (0_i64, 0_i64);

    for raw in &raw_rows {
        let row = derive_review_row(raw, run.id, &rule_hits, &rules_by_id)?;

        // 「全部」视图里，别处已确认且本所属期没有任何结论的行只是候选，不进查询结果
        if view == HistoryView::All
            && row.confirmed_elsewhere
            && raw.result_id.is_none()
            && !row.decision_manually_set
        {
            continue;
        }

        match row.match_state {
            MatchState::MissingVoucher => s.missing_voucher_count += 1,
            MatchState::Matched => s.matched_row_count += 1,
            MatchState::NotMatched => s.not_matched_count += 1,
        }
        if row.needs_manual_review && !row.locked {
            s.needs_review_count += 1;
        }
        if row.decision_source == DecisionSource::Manual {
            s.manual_count += 1;
        }
        if row.counted {
            s.included_row_count += 1;
            let tax = parse_fen(&row.final_transfer_tax)?;
            total_tax += tax;
            // 当前批次 = 尚未确认的，或在当前批次确认的；已锁定的旧批次不重复计入
            let in_current = match &row.confirm {
                None => true,
                Some(c) => c.same_run && c.batch_no == current.batch_no,
            };
            if in_current {
                s.batch_row_count += 1;
                batch_tax += tax;
            }
        }
        rows.push(row);
    }

    s.exists = true;
    s.src_from = Some(run.src_from.clone());
    s.src_to = Some(run.src_to.clone());
    s.state = current.state();
    s.batch_no = current.batch_no;
    s.total_row_count = to_u32(rows.len() as i64, "明细总条数")?;
    s.total_transfer_tax = total_tax.to_string();
    s.batch_transfer_tax = batch_tax.to_string();
    s.rows = rows;
    Ok(s)
}

fn filing_of(run: &RunRow) -> (&str, &str) {
    (run.filing_from.as_str(), run.filing_to.as_str())
}

fn apply_patch(
    tx: &Transaction<'_>,
    run: &RunRow,
    patch: &OverridePatch,
    updated_at: &str,
) -> Result<(), String> {
    let detail_id = i64::from(patch.detail_id);

    if !repository::detail_editable(tx, run.id, &run.src_from, &run.src_to, detail_id)? {
        return Err(format!(
            "发票明细 {detail_id} 不能修改（不在本纳税所属期的认证期间内，或已经确认）"
        ));
    }

    let transfer_tax = patch.transfer_tax.as_deref().map(parse_fen).transpose()?;

    if transfer_tax.is_some_and(|tax| tax < 0) {
        return Err("转出税额不能为负数".to_string());
    }

    repository::upsert_override(
        tx,
        run.id,
        detail_id,
        patch.included,
        transfer_tax,
        patch.remark.as_deref(),
        updated_at,
    )
}

/// 写操作的统一入口：取得已存在、且当前批次待确认的 run。
fn open_run(tx: &Transaction<'_>, filing_from: &str, filing_to: &str) -> Result<RunRow, String> {
    let (from, to) = period_repo::validate_filing(filing_from, filing_to)?;
    let run = period_repo::find_run(tx, MODULE, &from, &to)?
        .ok_or_else(|| format!("纳税所属期 {} 还没有匹配结果，请先匹配", period_repo::label(&from, &to)))?;
    period_repo::require_open(tx, &run)?;
    Ok(run)
}

// ───────────────── 命令 ─────────────────

/// 执行匹配：回填凭证号 → 取得（必要时创建）纳税所属期批次并校正认证期间 → 同步系统结果 → 返回完整核对列表。
/// 人工结论不受影响，人工处理过的明细不会被系统结论改写。
/// 批次 1 更改认证期间会清空本所属期已有的结果和人工修改，需 reset = true。
#[tauri::command]
#[specta::specta]
pub fn run_input_tax_transfer_match(
    request: MatchRequest,
    state: State<'_, DbState>,
    app: State<'_, AppRuntimeState>,
) -> Result<MatchRunResult, String> {
    let mut conn = state
        .conn
        .lock()
        .map_err(|_| "数据库连接锁定失败".to_string())?;

    let tx = conn
        .transaction()
        .map_err(|error| format!("开始匹配事务失败：{error}"))?;

    let ts = now();
    let run = period_repo::prepare_run_for_match(
        &tx,
        MODULE,
        (&request.filing_from, &request.filing_to),
        (&request.src_from, &request.src_to),
        request.reset,
        &app.username,
        &ts,
    )?;
    period_repo::require_open(&tx, &run)?;

    let voucher_filled_count =
        repository::backfill_voucher_no(&tx, &run.src_from, &run.src_to)?;
    let preserved_override_count = repository::count_overrides(&tx, run.id)?;

    let candidates = repository::fetch_match_candidates(&tx, &run.src_from, &run.src_to)?;
    let rules = repository::fetch_enabled_rules(&tx)?;

    let mut matched_item_count = 0_usize;
    let mut matched_detail_ids = HashSet::new();

    for candidate in &candidates {
        let input = MatchInput {
            seller_tax_no: Some(candidate.seller_tax_no.as_str()),
            tax_code: candidate.tax_code.as_deref(),
            goods_category: candidate.goods_category.as_deref(),
        };

        let rule_ids = match_engine::find_matching_rule_ids(&rules, &input);
        if rule_ids.is_empty() {
            continue;
        }

        repository::upsert_result(&tx, candidate, run.id, &ts, &rule_ids)?;
        matched_detail_ids.insert(candidate.detail_id);
        matched_item_count += 1;
    }

    // 本次不再命中的旧结果（例如规则被禁用、凭证被删）才删除，其余保留 id
    repository::delete_stale_results(&tx, run.id, &matched_detail_ids)?;

    let review = summarize_run(&tx, &run, HistoryView::All)?;

    tx.commit()
        .map_err(|error| format!("提交匹配事务失败：{error}"))?;

    Ok(MatchRunResult {
        voucher_filled_count: to_u32(voucher_filled_count as i64, "凭证号回填条数")?,
        qualified_item_count: to_u32(candidates.len() as i64, "待匹配明细条数")?,
        matched_item_count: to_u32(matched_item_count as i64, "命中明细条数")?,
        preserved_override_count: to_u32(preserved_override_count as i64, "人工结论条数")?,
        review,
    })
}

/// 只读加载核对列表，不重新匹配。用于切换纳税所属期与刷新。
#[tauri::command]
#[specta::specta]
pub fn load_input_tax_transfer_review(
    request: FilingRequest,
    state: State<'_, DbState>,
) -> Result<ReviewSummary, String> {
    let mut conn = state
        .conn
        .lock()
        .map_err(|_| "数据库连接锁定失败".to_string())?;
    load_review(&mut conn, &request, HistoryView::All)
}

pub(crate) fn load_review(
    conn: &mut rusqlite::Connection,
    request: &FilingRequest,
    view: HistoryView,
) -> Result<ReviewSummary, String> {
    let (from, to) = period_repo::validate_filing(&request.filing_from, &request.filing_to)?;
    let tx = conn
        .transaction()
        .map_err(|error| format!("开始查询事务失败：{error}"))?;
    let review = build_review_summary(&tx, (&from, &to), view)?;
    tx.commit()
        .map_err(|error| format!("提交查询事务失败：{error}"))?;
    Ok(review)
}

/// 查询页：与 [from, to] 有交集的每个纳税所属期，各返回批次状态和明细。
/// `Confirmed` 只含已确认/已锁定的明细；`All` 含本所属期所有已入库的结果（命中 + 人工修改）。
#[tauri::command]
#[specta::specta]
pub fn query_input_tax_transfer_history(
    period_from: String,
    period_to: String,
    view: HistoryView,
    state: State<'_, DbState>,
) -> Result<Vec<TransferHistoryPeriod>, String> {
    let mut conn = state
        .conn
        .lock()
        .map_err(|_| "数据库连接锁定失败".to_string())?;
    load_history(&mut conn, &period_from, &period_to, view)
}

pub(crate) fn load_history(
    conn: &mut rusqlite::Connection,
    period_from: &str,
    period_to: &str,
    view: HistoryView,
) -> Result<Vec<TransferHistoryPeriod>, String> {
    let (from, to) = period_repo::validate_range(period_from, period_to, "纳税所属期")?;
    let tx = conn
        .transaction()
        .map_err(|error| format!("开始查询事务失败：{error}"))?;

    let mut periods = Vec::new();
    for run in period_repo::runs_overlapping(&tx, MODULE, &from, &to)? {
        periods.push(TransferHistoryPeriod { review: summarize_run(&tx, &run, view)? });
    }
    tx.commit()
        .map_err(|error| format!("提交查询事务失败：{error}"))?;
    Ok(periods)
}

/// 保存单条人工修改，返回刷新后的整体汇总。
#[tauri::command]
#[specta::specta]
pub fn save_input_tax_transfer_override(
    request: SaveOverrideRequest,
    state: State<'_, DbState>,
) -> Result<ReviewSummary, String> {
    let mut conn = state
        .conn
        .lock()
        .map_err(|_| "数据库连接锁定失败".to_string())?;

    let tx = conn
        .transaction()
        .map_err(|error| format!("开始保存事务失败：{error}"))?;

    let run = open_run(&tx, &request.filing_from, &request.filing_to)?;
    apply_patch(&tx, &run, &request.patch, &now())?;

    let review = summarize_run(&tx, &run, HistoryView::All)?;

    tx.commit()
        .map_err(|error| format!("提交保存事务失败：{error}"))?;

    Ok(review)
}

/// 批量保存人工修改，用于「批量标记转出」「批量恢复系统结论」等操作。
/// 整批在同一事务内提交，任一条失败则全部回滚。
#[tauri::command]
#[specta::specta]
pub fn save_input_tax_transfer_overrides_batch(
    request: SaveOverridesBatchRequest,
    state: State<'_, DbState>,
) -> Result<ReviewSummary, String> {
    if request.items.is_empty() {
        return Err("没有需要保存的明细".to_string());
    }

    let mut conn = state
        .conn
        .lock()
        .map_err(|_| "数据库连接锁定失败".to_string())?;

    let tx = conn
        .transaction()
        .map_err(|error| format!("开始批量保存事务失败：{error}"))?;

    let run = open_run(&tx, &request.filing_from, &request.filing_to)?;
    let updated_at = now();

    for patch in &request.items {
        apply_patch(&tx, &run, patch, &updated_at)?;
    }

    let review = summarize_run(&tx, &run, HistoryView::All)?;

    tx.commit()
        .map_err(|error| format!("提交批量保存事务失败：{error}"))?;

    Ok(review)
}

/// 确认当前批次：把本所属期尚未确认、且最终转出的明细写入已确认快照；之后只读，可导出清单。
/// 补充批次只含新增的明细（批次 1 已锁定的明细不会重复写入）。
#[tauri::command]
#[specta::specta]
pub fn confirm_input_tax_transfer(
    request: PeriodActionRequest,
    state: State<'_, DbState>,
    app: State<'_, AppRuntimeState>,
) -> Result<PeriodStatus, String> {
    if request.module != MODULE {
        return Err("模块不匹配".to_string());
    }
    let mut conn = state
        .conn
        .lock()
        .map_err(|_| "数据库连接锁定失败".to_string())?;
    let tx = conn
        .transaction()
        .map_err(|error| format!("开始确认事务失败：{error}"))?;

    let run = open_run(&tx, &request.filing_from, &request.filing_to)?;
    let batch = period_repo::current_batch(&tx, run.id)?;
    let review = summarize_run(&tx, &run, HistoryView::All)?;

    let ts = now();
    for row in review.rows.iter().filter(|row| row.counted && !row.locked) {
        let tax = parse_fen(&row.final_transfer_tax)?;
        // 人工改了转出结论或税额 → manual；只写了备注 → reviewed；其余 → auto
        // （与系统一致的人工结论在保存时已被清掉，见 drop_if_same_as_system）
        let decision = if row.decision_manually_set || row.tax_manually_edited {
            "manual"
        } else if row.remark.is_some() {
            "reviewed"
        } else {
            "auto"
        };
        repository::insert_confirmed_line(
            &tx,
            run.id,
            batch.id,
            &ConfirmLine {
                detail_id: i64::from(row.detail_id),
                header_id: i64::from(row.header_id),
                cert_period: row.cert_period.clone(),
                detail_tax: parse_fen(&row.tax)?,
                transfer_tax: tax,
                decision,
                remark: row.remark.clone(),
            },
            &ts,
        )?;
    }
    period_repo::mark_confirmed(&tx, batch.id, &app.username, &ts)?;

    let status = period_repo::status(&tx, MODULE, &run.filing_from, &run.filing_to, app.is_leader)?;
    tx.commit()
        .map_err(|error| format!("提交确认事务失败：{error}"))?;
    Ok(status)
}
