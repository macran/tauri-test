use std::collections::{HashMap, HashSet};

use chrono::Local;
use rusqlite::Transaction;
use tauri::State;

use crate::features::period_close::repository::{self as period_repo, RunRow};
use crate::features::period_close::{BatchState, PeriodActionRequest, PeriodStatus};
use crate::{AppRuntimeState, DbState};

use super::match_engine::{self, MatchInput};
use super::models::{
    stamp_amount, stamp_base, RawReviewRow, RuleForMatch, StampHistoryPeriod, StampHistoryView,
    StampTaxDecisionSource, StampTaxMatchRequest, StampTaxMatchRunResult, StampTaxMatchState,
    StampTaxMatchedRule, StampTaxOverridePatch, StampTaxReviewRow, StampTaxReviewSummary,
    StampTaxSaveOverrideRequest, StampTaxSaveOverridesBatchRequest,
};
use super::repository::{self, ConfirmLine};
use crate::features::period_close::FilingRequest;

const MODULE: &str = "stamp_tax";

// ───────────────── 校验与转换 ─────────────────

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
    categories: &HashMap<String, (String, Option<i64>)>,
) -> Result<StampTaxReviewRow, String> {
    let system_matched = raw.result_id.is_some();

    // 命中规则不受人工结论影响：只要系统本次命中，条件与备注就一直可见。
    let matched_rules: Vec<StampTaxMatchedRule> = raw
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

    // 命中多条规则无法唯一确定税目，一律需人工核对。
    let needs_manual_review =
        matched_rules.len() > 1 || matched_rules.iter().any(|rule| rule.needs_manual_review);

    let override_included = raw.override_included.map(|value| value != 0);
    let decision_manually_set = raw.override_included.is_some();
    let manual_overridden = decision_manually_set || raw.override_remark.is_some();

    let match_state = if system_matched {
        StampTaxMatchState::Matched
    } else {
        StampTaxMatchState::NotMatched
    };

    let confirm = raw
        .confirm
        .as_ref()
        .map(|c| period_repo::confirm_info(c, current_run_id));
    let confirmed_elsewhere = confirm.as_ref().is_some_and(|c| !c.same_run);

    // 计税基数：专用发票用金额，其余票种用价税合计（现有逻辑，勿另写一份）。
    let live_base = stamp_base(raw.inv_type, raw.amount, raw.total);

    // 已确认的明细以确认时的快照为准（类别、基数、税率、税额），不再随规则或人工变化。
    let snapshot = match (&raw.confirm, &raw.confirmed_category) {
        (Some(_), Some(code)) => Some((
            code.clone(),
            raw.confirmed_base.unwrap_or(0),
            raw.confirmed_rate,
            raw.confirmed_tax.unwrap_or(0),
        )),
        _ => None,
    };

    let hit_rules: Vec<&RuleForMatch> = raw
        .result_id
        .and_then(|result_id| rule_hits.get(&result_id))
        .into_iter()
        .flatten()
        .filter_map(|rule_id| rules_by_id.get(rule_id).copied())
        .filter(|rule| rule.stamp_tax_category_code.is_some())
        .collect();
    let multi_hit = hit_rules.len() > 1;

    let (decision_source, final_included, category, base, tax) = if let Some((code, base, rate, tax)) =
        snapshot
    {
        let source = match raw.confirmed_decision.as_deref() {
            Some("manual") => StampTaxDecisionSource::Manual,
            _ => StampTaxDecisionSource::System,
        };
        let name = categories.get(&code).map(|(name, _)| name.clone()).unwrap_or_else(|| code.clone());
        (source, true, Some((code, name, rate.unwrap_or(0))), base, tax)
    } else {
        let source = if manual_overridden { StampTaxDecisionSource::Manual } else { StampTaxDecisionSource::System };
        let included = override_included.unwrap_or(system_matched);

        // 印花税类别：人工指定优先；否则仅命中一条规则时沿用，命中多条则留空待用户选定。
        let category_code = match &raw.override_category_code {
            Some(code) => Some(code.clone()),
            None if included && !multi_hit => hit_rules
                .first()
                .and_then(|rule| rule.stamp_tax_category_code.clone()),
            None => None,
        };
        // rate 为 NULL 的是必须选子目的父税目，视为未选定
        let category = category_code.and_then(|code| {
            categories
                .get(&code)
                .and_then(|(name, rate)| rate.map(|rate| (code.clone(), name.clone(), rate)))
        });
        let tax = category
            .as_ref()
            .filter(|_| included)
            .map_or(0, |(_, _, rate)| stamp_amount(live_base, *rate));
        (source, included, category, live_base, tax)
    };

    let locked = confirm.is_some();
    let needs_category_choice = !locked && final_included && category.is_none();

    Ok(StampTaxReviewRow {
        detail_id: to_u32(raw.detail_id, "发票明细 ID")?,
        header_id: to_u32(raw.header_id, "发票头 ID")?,
        line_no: to_u32(raw.line_no, "明细行号")?,
        booked_period: raw.booked_period.clone(),
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
        stamp_base: base.to_string(),
        stamp_tax_amount: tax.to_string(),
        match_state,
        decision_source,
        needs_manual_review,
        decision_manually_set,
        final_included,
        stamp_tax_category_code: category.as_ref().map(|(code, _, _)| code.clone()),
        stamp_tax_category: category.as_ref().map(|(_, name, _)| name.clone()),
        rate_ppm: category.as_ref().map(|(_, _, rate)| to_u32(*rate, "税率")).transpose()?,
        category_manually_set: raw.override_category_code.is_some(),
        needs_category_choice,
        remark: raw.override_remark.clone(),
        matched_rules,
        counted: final_included && !confirmed_elsewhere,
        locked,
        confirmed_elsewhere,
        confirm,
    })
}

fn empty_summary(filing: (&str, &str)) -> StampTaxReviewSummary {
    StampTaxReviewSummary {
        filing_from: filing.0.to_string(),
        filing_to: filing.1.to_string(),
        exists: false,
        src_from: None,
        src_to: None,
        state: BatchState::Open,
        batch_no: 1,
        total_row_count: 0,
        matched_row_count: 0,
        not_matched_count: 0,
        needs_review_count: 0,
        manual_count: 0,
        included_row_count: 0,
        unresolved_count: 0,
        total_stamp_tax: "0".to_string(),
        batch_row_count: 0,
        batch_stamp_tax: "0".to_string(),
        rows: Vec::new(),
    }
}

fn build_review_summary(
    tx: &Transaction<'_>,
    filing: (&str, &str),
    view: StampHistoryView,
) -> Result<StampTaxReviewSummary, String> {
    let Some(run) = period_repo::find_run(tx, MODULE, filing.0, filing.1)? else {
        return Ok(empty_summary(filing));
    };
    summarize_run(tx, &run, view)
}

fn summarize_run(
    tx: &Transaction<'_>,
    run: &RunRow,
    view: StampHistoryView,
) -> Result<StampTaxReviewSummary, String> {
    let current = period_repo::current_batch(tx, run.id)?;
    let raw_rows = repository::fetch_review_rows(
        tx,
        run.id,
        &run.src_from,
        &run.src_to,
        view == StampHistoryView::Confirmed,
    )?;

    let result_ids: Vec<i64> = raw_rows.iter().filter_map(|row| row.result_id).collect();
    let rule_hits = repository::fetch_rule_hits(tx, &result_ids)?;

    // 禁用的规则也读出来，历史命中仍需展示条件与备注。
    let all_rules = repository::fetch_rules(tx, false)?;
    let rules_by_id: HashMap<i64, &RuleForMatch> =
        all_rules.iter().map(|rule| (rule.id, rule)).collect();
    let categories = repository::fetch_categories(tx)?;

    let mut s = empty_summary((&run.filing_from, &run.filing_to));
    let mut rows = Vec::with_capacity(raw_rows.len());
    let (mut total_tax, mut batch_tax) = (0_i64, 0_i64);

    for raw in &raw_rows {
        let row = derive_review_row(raw, run.id, &rule_hits, &rules_by_id, &categories)?;

        // 「全部」视图里，别处已确认且本所属期没有任何结论的行只是候选，不进查询结果
        if view == StampHistoryView::All
            && row.confirmed_elsewhere
            && raw.result_id.is_none()
            && !row.decision_manually_set
        {
            continue;
        }

        match row.match_state {
            StampTaxMatchState::Matched => s.matched_row_count += 1,
            StampTaxMatchState::NotMatched => s.not_matched_count += 1,
        }
        if row.needs_manual_review && !row.locked {
            s.needs_review_count += 1;
        }
        if row.decision_source == StampTaxDecisionSource::Manual {
            s.manual_count += 1;
        }
        if row.needs_category_choice {
            s.unresolved_count += 1;
        }
        if row.counted {
            s.included_row_count += 1;
            let tax: i64 = row.stamp_tax_amount.parse().unwrap_or(0);
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
    s.total_stamp_tax = total_tax.to_string();
    s.batch_stamp_tax = batch_tax.to_string();
    s.rows = rows;
    Ok(s)
}

fn apply_patch(
    tx: &Transaction<'_>,
    run: &RunRow,
    patch: &StampTaxOverridePatch,
    updated_at: &str,
) -> Result<(), String> {
    let detail_id = i64::from(patch.detail_id);

    if !repository::detail_editable(tx, run.id, &run.src_from, &run.src_to, detail_id)? {
        return Err(format!(
            "发票明细 {detail_id} 不能修改（凭证入账期间不在本纳税所属期的范围内，或已经确认）"
        ));
    }

    repository::upsert_override(
        tx,
        run.id,
        detail_id,
        patch.included,
        patch.stamp_tax_category_code.as_deref(),
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

/// 执行匹配：取得（必要时创建）纳税所属期批次并校正凭证期间 → 同步系统结果 → 返回完整核对列表。
/// 人工结论表不受影响，人工处理过的明细不会被系统结论改写。
/// 批次 1 更改凭证期间会清空本所属期已有的结果和人工修改，需 reset = true。
#[tauri::command]
#[specta::specta]
pub fn run_stamp_tax_match(
    request: StampTaxMatchRequest,
    state: State<'_, DbState>,
    app: State<'_, AppRuntimeState>,
) -> Result<StampTaxMatchRunResult, String> {
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

    // 本次不再命中的旧结果才删除，其余保留 id
    repository::delete_stale_results(&tx, run.id, &matched_detail_ids)?;

    let review = summarize_run(&tx, &run, StampHistoryView::All)?;

    tx.commit()
        .map_err(|error| format!("提交匹配事务失败：{error}"))?;

    Ok(StampTaxMatchRunResult {
        qualified_item_count: to_u32(candidates.len() as i64, "待匹配明细条数")?,
        matched_item_count: to_u32(matched_item_count as i64, "命中明细条数")?,
        preserved_override_count: to_u32(preserved_override_count as i64, "人工结论条数")?,
        review,
    })
}

/// 只读加载核对列表，不重新匹配。用于切换纳税所属期与刷新。
#[tauri::command]
#[specta::specta]
pub fn load_stamp_tax_review(
    request: FilingRequest,
    state: State<'_, DbState>,
) -> Result<StampTaxReviewSummary, String> {
    let mut conn = state
        .conn
        .lock()
        .map_err(|_| "数据库连接锁定失败".to_string())?;
    load_review(&mut conn, &request, StampHistoryView::All)
}

pub(crate) fn load_review(
    conn: &mut rusqlite::Connection,
    request: &FilingRequest,
    view: StampHistoryView,
) -> Result<StampTaxReviewSummary, String> {
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
#[tauri::command]
#[specta::specta]
pub fn query_stamp_tax_history(
    period_from: String,
    period_to: String,
    view: StampHistoryView,
    state: State<'_, DbState>,
) -> Result<Vec<StampHistoryPeriod>, String> {
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
    view: StampHistoryView,
) -> Result<Vec<StampHistoryPeriod>, String> {
    let (from, to) = period_repo::validate_range(period_from, period_to, "纳税所属期")?;
    let tx = conn
        .transaction()
        .map_err(|error| format!("开始查询事务失败：{error}"))?;
    let mut periods = Vec::new();
    for run in period_repo::runs_overlapping(&tx, MODULE, &from, &to)? {
        periods.push(StampHistoryPeriod { review: summarize_run(&tx, &run, view)? });
    }
    tx.commit()
        .map_err(|error| format!("提交查询事务失败：{error}"))?;
    Ok(periods)
}

/// 保存单条人工修改，返回刷新后的整体汇总。
#[tauri::command]
#[specta::specta]
pub fn save_stamp_tax_override(
    request: StampTaxSaveOverrideRequest,
    state: State<'_, DbState>,
) -> Result<StampTaxReviewSummary, String> {
    let mut conn = state
        .conn
        .lock()
        .map_err(|_| "数据库连接锁定失败".to_string())?;

    let tx = conn
        .transaction()
        .map_err(|error| format!("开始保存事务失败：{error}"))?;

    let run = open_run(&tx, &request.filing_from, &request.filing_to)?;
    apply_patch(&tx, &run, &request.patch, &now())?;

    let review = summarize_run(&tx, &run, StampHistoryView::All)?;

    tx.commit()
        .map_err(|error| format!("提交保存事务失败：{error}"))?;

    Ok(review)
}

/// 批量保存人工修改。整批在同一事务内提交，任一条失败则全部回滚。
#[tauri::command]
#[specta::specta]
pub fn save_stamp_tax_overrides_batch(
    request: StampTaxSaveOverridesBatchRequest,
    state: State<'_, DbState>,
) -> Result<StampTaxReviewSummary, String> {
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

    let review = summarize_run(&tx, &run, StampHistoryView::All)?;

    tx.commit()
        .map_err(|error| format!("提交批量保存事务失败：{error}"))?;

    Ok(review)
}

/// 确认当前批次：把本所属期尚未确认、且最终计入的明细写入已确认快照；之后只读，可导出采集表。
/// 「需人工核对」只是提示，不挡确认（用户没取消计入的就按计入走）；但计入的明细必须有明确的印花税类别，
/// 命中多个类别又没人工选定的，算不出税，仍须先处理。补充批次只含新增的明细。
#[tauri::command]
#[specta::specta]
pub fn confirm_stamp_tax(
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
    let review = summarize_run(&tx, &run, StampHistoryView::All)?;

    if review.unresolved_count > 0 {
        return Err(format!(
            "还有 {} 条计入明细未指定有效的印花税类别，处理完才能确认",
            review.unresolved_count
        ));
    }

    let ts = now();
    for row in review.rows.iter().filter(|row| row.counted && !row.locked) {
        let code = row
            .stamp_tax_category_code
            .clone()
            .ok_or_else(|| format!("明细 {} 缺少印花税类别", row.detail_id))?;
        let rate = row
            .rate_ppm
            .ok_or_else(|| format!("明细 {} 缺少税率", row.detail_id))?;
        // 人工改了计入结论或类别 → manual；只写了备注 → reviewed；其余 → auto
        let decision = if row.decision_manually_set || row.category_manually_set {
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
                category_code: code,
                base_amount: row.stamp_base.parse().map_err(|_| "计税基数格式错误".to_string())?,
                rate_ppm: i64::from(rate),
                stamp_tax: row.stamp_tax_amount.parse().map_err(|_| "印花税额格式错误".to_string())?,
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

// ───────────────── 印花税税源信息采集表 ─────────────────

use super::report::{self, DeclareType, PeriodType, ReportLine, StampTaxReport};
use serde::Deserialize;

/// 导出采集表的请求。申报类型默认 11 正常申报、申报期限类型默认 00 按期申报，由用户在界面选择。
#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CollectionFormRequest {
    pub filing_from: String,
    pub filing_to: String,
    /// 导出哪个批次：None = 当前（编号最大）批次
    pub batch_no: Option<u32>,
    pub declare_type: DeclareType,
    pub period_type: PeriodType,
    /// 保存路径（由 dialog 选好）
    pub path: String,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CollectionFormResult {
    pub path: String,
    pub row_count: u32,
    /// 计税金额合计（分）
    pub total_taxable: String,
    /// 应纳税额合计（分），按子目汇总后计算
    pub total_tax: String,
}

/// 采集表默认文件名：`{纳税所属期}_印花税税源信息采集表.xlsx`；补充批次加「_补充」。
/// 纳税所属期取该批次 run 的所属期，如 202610 或 202601-202603。
#[tauri::command]
#[specta::specta]
pub fn stamp_tax_collection_form_filename(filing_from: String, filing_to: String, batch_no: u32) -> String {
    collection_form_filename(&filing_from, &filing_to, batch_no)
}

pub(crate) fn collection_form_filename(filing_from: &str, filing_to: &str, batch_no: u32) -> String {
    let period = period_repo::label(filing_from, filing_to);
    let suffix = if batch_no > 1 { "_补充" } else { "" };
    format!("{period}_印花税税源信息采集表{suffix}.xlsx")
}

/// 按纳税所属期导出印花税税源信息采集表。只汇总所选批次**已确认**的明细：
/// 批次 1 = 原申报清单；补充批次 = 只含补充的明细。
#[tauri::command]
#[specta::specta]
pub async fn export_stamp_tax_collection_form(
    request: CollectionFormRequest,
    state: State<'_, DbState>,
) -> Result<CollectionFormResult, String> {
    if !request.path.to_ascii_lowercase().ends_with(".xlsx") {
        return Err("导出文件必须是 .xlsx".to_string());
    }
    let (from, to) = period_repo::validate_filing(&request.filing_from, &request.filing_to)?;

    let lines = {
        let mut conn = state
            .conn
            .lock()
            .map_err(|_| "数据库连接锁定失败".to_string())?;
        let tx = conn
            .transaction()
            .map_err(|error| format!("开始导出事务失败：{error}"))?;
        let run = period_repo::find_run(&tx, MODULE, &from, &to)?
            .ok_or_else(|| format!("纳税所属期 {} 还没有匹配结果", period_repo::label(&from, &to)))?;
        let batches = period_repo::batches(&tx, run.id)?;
        let batch = match request.batch_no {
            Some(no) => batches.iter().find(|b| b.batch_no == no),
            None => batches.last(),
        }
        .ok_or("批次不存在")?;
        if batch.state() == BatchState::Open {
            return Err(format!("批次 {} 还没有确认，请先确认再导出采集表", batch.batch_no));
        }
        repository::collection_lines(&tx, batch.id)?
    };

    if lines.is_empty() {
        return Err("该批次没有计入印花税的明细，无需导出采集表".to_string());
    }

    let first_day = period_repo::period_first_day(&from);
    let last_day = period_repo::period_last_day(&to);
    let parse = |d: &str| {
        chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").map_err(|_| format!("日期无效：{d}"))
    };
    let report_input = StampTaxReport {
        period_start: parse(&first_day)?,
        period_end: parse(&last_day)?,
        declare_type: request.declare_type,
        lines: lines
            .into_iter()
            .map(|l| ReportLine {
                period_type: request.period_type,
                item_code: l.item_code,
                item_name: l.item_name,
                sub_item_code: l.sub_item_code,
                sub_item_name: l.sub_item_name,
                rate_ppm: l.rate_ppm,
                taxable_fen: l.taxable_fen,
                voucher_name: None,
            })
            .collect(),
    };

    let path = request.path.clone();
    let summary = tauri::async_runtime::spawn_blocking(move || {
        report::generate(report::TEMPLATE, &report_input, std::path::Path::new(&path))
    })
    .await
    .map_err(|error| format!("生成采集表失败：{error}"))?
    .map_err(|error| error.to_string())?;

    Ok(CollectionFormResult {
        path: request.path,
        row_count: summary.row_count as u32,
        total_taxable: summary.total_taxable_fen.to_string(),
        total_tax: summary.total_tax_fen.to_string(),
    })
}

#[cfg(test)]
mod report_tests {
    use super::collection_form_filename;

    #[test]
    fn filename_uses_filing_period_and_marks_supplement() {
        assert_eq!(collection_form_filename("202610", "202610", 1), "202610_印花税税源信息采集表.xlsx");
        assert_eq!(
            collection_form_filename("202601", "202603", 1),
            "202601-202603_印花税税源信息采集表.xlsx"
        );
        assert_eq!(
            collection_form_filename("202601", "202603", 2),
            "202601-202603_印花税税源信息采集表_补充.xlsx"
        );
    }
}
