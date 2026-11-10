use std::collections::HashMap;

use chrono::Local;
use rusqlite::Transaction;
use tauri::State;

use crate::DbState;

use super::match_engine::{self, MatchInput};
use super::models::{
    DecisionSource, MatchRunResult, MatchState, MatchedRule, OverridePatch, PeriodRangeRequest,
    RawReviewRow, ReviewRow, ReviewSummary, RuleForMatch, SaveOverrideRequest,
    SaveOverridesBatchRequest,
};
use super::repository;

// ───────────────── 校验与转换 ─────────────────

fn validate_period(value: &str) -> Result<String, String> {
    let trimmed = value.trim();

    if trimmed.len() != 6 || !trimmed.chars().all(|c| c.is_ascii_digit()) {
        return Err("期间格式错误，应为 6 位数字，例如 202601".to_string());
    }

    let month: u32 = trimmed[4..6]
        .parse()
        .map_err(|_| "期间月份无效".to_string())?;

    if !(1..=12).contains(&month) {
        return Err("期间月份无效，应在 01~12 之间".to_string());
    }

    Ok(trimmed.to_string())
}

/// 单期时前端传相同的起止；选两期表示闭区间，顺序颠倒时自动纠正。
fn validate_range(period_from: &str, period_to: &str) -> Result<(String, String), String> {
    let from = validate_period(period_from)?;
    let to = validate_period(period_to)?;

    if from <= to {
        Ok((from, to))
    } else {
        Ok((to, from))
    }
}

fn parse_fen(value: &str) -> Result<i64, String> {
    let trimmed = value.trim();
    trimmed
        .parse::<i64>()
        .map_err(|_| format!("转出税额格式错误：{trimmed}"))
}

fn to_u32(value: i64, label: &str) -> Result<u32, String> {
    u32::try_from(value).map_err(|_| format!("{label}超出可导出范围"))
}

// ───────────────── 派生核对行 ─────────────────

fn derive_review_row(
    raw: &RawReviewRow,
    rule_hits: &HashMap<i64, Vec<i64>>,
    rules_by_id: &HashMap<i64, &RuleForMatch>,
) -> Result<ReviewRow, String> {
    let system_matched = raw.result_id.is_some();

    // 命中规则不受人工结论影响：只要系统本次命中，条件与备注就一直可见。
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

    // 结论来源：人工动过任一维度即为人工结论，重新匹配不会覆盖。
    let decision_source = if manual_overridden {
        DecisionSource::Manual
    } else {
        DecisionSource::System
    };

    // 最终结论：人工判定优先，未表态则沿用系统命中结果。
    let final_included = override_included.unwrap_or(system_matched);

    // 最终税额：人工税额优先；否则转出取明细税额，不转出取 0。
    let final_transfer_tax = raw
        .override_transfer_tax
        .unwrap_or(if final_included { raw.tax } else { 0 });

    Ok(ReviewRow {
        detail_id: to_u32(raw.detail_id, "发票明细 ID")?,
        header_id: to_u32(raw.header_id, "发票头 ID")?,
        line_no: to_u32(raw.line_no, "明细行号")?,
        tax_period: raw.tax_period.clone(),
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
    })
}

fn build_review_summary(
    tx: &Transaction<'_>,
    period_from: &str,
    period_to: &str,
) -> Result<ReviewSummary, String> {
    let raw_rows = repository::fetch_review_rows(tx, period_from, period_to)?;

    let result_ids: Vec<i64> = raw_rows.iter().filter_map(|row| row.result_id).collect();
    let rule_hits = repository::fetch_rule_hits(tx, &result_ids)?;

    // 禁用的规则也读出来，历史命中仍需展示条件与备注。
    let all_rules = repository::fetch_rules(tx, false)?;
    let rules_by_id: HashMap<i64, &RuleForMatch> =
        all_rules.iter().map(|rule| (rule.id, rule)).collect();

    let mut rows = Vec::with_capacity(raw_rows.len());
    let mut missing_voucher_count = 0_u32;
    let mut matched_row_count = 0_u32;
    let mut not_matched_count = 0_u32;
    let mut needs_review_count = 0_u32;
    let mut manual_count = 0_u32;
    let mut included_row_count = 0_u32;
    let mut total_transfer_tax = 0_i64;

    for raw in &raw_rows {
        let row = derive_review_row(raw, &rule_hits, &rules_by_id)?;

        match row.match_state {
            MatchState::MissingVoucher => missing_voucher_count += 1,
            MatchState::Matched => matched_row_count += 1,
            MatchState::NotMatched => not_matched_count += 1,
        }

        if row.needs_manual_review {
            needs_review_count += 1;
        }
        if row.decision_source == DecisionSource::Manual {
            manual_count += 1;
        }
        if row.final_included {
            included_row_count += 1;
            total_transfer_tax += parse_fen(&row.final_transfer_tax)?;
        }

        rows.push(row);
    }

    Ok(ReviewSummary {
        period_from: period_from.to_string(),
        period_to: period_to.to_string(),
        total_row_count: to_u32(rows.len() as i64, "明细总条数")?,
        missing_voucher_count,
        matched_row_count,
        not_matched_count,
        needs_review_count,
        manual_count,
        included_row_count,
        total_transfer_tax: total_transfer_tax.to_string(),
        rows,
    })
}

fn apply_patch(
    tx: &Transaction<'_>,
    period_from: &str,
    period_to: &str,
    patch: &OverridePatch,
    updated_at: &str,
) -> Result<(), String> {
    let detail_id = i64::from(patch.detail_id);

    if !repository::detail_in_period(tx, period_from, period_to, detail_id)? {
        return Err(format!("发票明细 {detail_id} 不属于所选期间的认证清单"));
    }

    let transfer_tax = patch.transfer_tax.as_deref().map(parse_fen).transpose()?;

    if transfer_tax.is_some_and(|tax| tax < 0) {
        return Err("转出税额不能为负数".to_string());
    }

    repository::upsert_override(
        tx,
        period_from,
        period_to,
        detail_id,
        patch.included,
        transfer_tax,
        patch.remark.as_deref(),
        updated_at,
    )
}

// ───────────────── 命令 ─────────────────

/// 执行匹配：回填凭证号 → 重建系统结果 → 返回完整核对列表。
/// 人工结论表不受影响，人工处理过的明细不会被系统结论改写。
#[tauri::command]
#[specta::specta]
pub fn run_input_tax_transfer_match(
    request: PeriodRangeRequest,
    state: State<'_, DbState>,
) -> Result<MatchRunResult, String> {
    let (period_from, period_to) = validate_range(&request.period_from, &request.period_to)?;

    let mut conn = state
        .conn
        .lock()
        .map_err(|_| "数据库连接锁定失败".to_string())?;

    let tx = conn
        .transaction()
        .map_err(|error| format!("开始匹配事务失败：{error}"))?;

    let voucher_filled_count = repository::backfill_voucher_no(&tx, &period_from, &period_to)?;
    let preserved_override_count = repository::count_overrides(&tx, &period_from, &period_to)?;

    let candidates = repository::fetch_match_candidates(&tx, &period_from, &period_to)?;
    let rules = repository::fetch_enabled_rules(&tx)?;

    repository::clear_results(&tx, &period_from, &period_to)?;

    let matched_at = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let mut matched_item_count = 0_usize;

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

        repository::insert_result(
            &tx,
            candidate,
            &period_from,
            &period_to,
            &matched_at,
            &rule_ids,
        )?;
        matched_item_count += 1;
    }

    let review = build_review_summary(&tx, &period_from, &period_to)?;

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

/// 只读加载核对列表，不重新匹配。用于切换期间与刷新。
#[tauri::command]
#[specta::specta]
pub fn load_input_tax_transfer_review(
    request: PeriodRangeRequest,
    state: State<'_, DbState>,
) -> Result<ReviewSummary, String> {
    let (period_from, period_to) = validate_range(&request.period_from, &request.period_to)?;

    let mut conn = state
        .conn
        .lock()
        .map_err(|_| "数据库连接锁定失败".to_string())?;

    let tx = conn
        .transaction()
        .map_err(|error| format!("开始查询事务失败：{error}"))?;

    let review = build_review_summary(&tx, &period_from, &period_to)?;

    tx.commit()
        .map_err(|error| format!("提交查询事务失败：{error}"))?;

    Ok(review)
}

/// 保存单条人工修改，返回刷新后的整体汇总。
#[tauri::command]
#[specta::specta]
pub fn save_input_tax_transfer_override(
    request: SaveOverrideRequest,
    state: State<'_, DbState>,
) -> Result<ReviewSummary, String> {
    let (period_from, period_to) = validate_range(&request.period_from, &request.period_to)?;

    let mut conn = state
        .conn
        .lock()
        .map_err(|_| "数据库连接锁定失败".to_string())?;

    let tx = conn
        .transaction()
        .map_err(|error| format!("开始保存事务失败：{error}"))?;

    let updated_at = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    apply_patch(&tx, &period_from, &period_to, &request.patch, &updated_at)?;

    let review = build_review_summary(&tx, &period_from, &period_to)?;

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
    let (period_from, period_to) = validate_range(&request.period_from, &request.period_to)?;

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

    let updated_at = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

    for patch in &request.items {
        apply_patch(&tx, &period_from, &period_to, patch, &updated_at)?;
    }

    let review = build_review_summary(&tx, &period_from, &period_to)?;

    tx.commit()
        .map_err(|error| format!("提交批量保存事务失败：{error}"))?;

    Ok(review)
}