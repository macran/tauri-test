//! 进项税转出
//!
//! 数据范围：certified_list.tax_period ∈ 认证期间范围，且发票在 AP / IC / 银行 / 差旅
//!           四种凭证中至少有一条凭证（v_invoice_vouchers）
//! 匹配：逐条评估 scope = 'input_tax_transfer' 的启用规则，命中的全部写入 hits
//! 人工：overrides 中有 included / transfer_tax 判定的明细，重新匹配时不再参与
//! 默认值：转出 = 命中；转出税额 = 明细税额

use std::collections::HashMap;

use anyhow::{anyhow, bail, Result};
use rusqlite::{params, Connection, OptionalExtension};
use rust_xlsxwriter::Workbook;
use serde::{Deserialize, Serialize};
use specta::Type;

use super::common::*;
use super::excel::{col, write_info_sheet, write_table, Col, Val};
use super::rule_engine::{self, RuleInput};
use super::{INVOICE_NO_SQL, LIKE_ESCAPE};

const M: TaxModule = TaxModule::InputTaxTransfer;
const SCOPE: &str = "input_tax_transfer";

// ─────────────────────────────────────────────────────────────
// 类型
// ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TransferLine {
    pub result_id: Option<i64>,
    pub matched_at: Option<String>,
    pub detail_id: i64,
    pub header_id: i64,
    pub cert_period: String,
    pub invoice_code: String,
    pub invoice_no: String,
    pub issue_date: String,
    pub seller_name: String,
    pub seller_tax_no: String,
    pub line_no: i64,
    pub tax_code: Option<String>,
    pub goods_category: Option<String>,
    pub goods_name: String,
    pub spec_model: Option<String>,
    pub amount: i64,
    pub tax_rate: Option<String>,
    pub tax: i64,
    pub voucher_nos: Option<String>,
    pub voucher_dates: Option<String>,
    pub has_voucher: bool,
    pub auto_hit: bool,
    /// 系统已在其他纳税所属期命中过；当前批次不重复跑规则，但仍可人工复核。
    pub matched_in_other_period: bool,
    pub hit_rule_ids: Vec<i64>,
    pub hit_categories: Vec<String>,
    pub hit_rules: Option<String>,
    pub override_included: Option<bool>,
    pub override_tax: Option<i64>,
    pub remark: Option<String>,
    pub updated_by: Option<String>,
    pub updated_at: Option<String>,
    pub final_included: bool,
    pub final_tax: i64,
    pub match_type: MatchType,
    pub needs_review: bool,
    pub confirmed_batch_no: Option<i64>,
    pub locked: bool,
    /// 本批待确认：转出且尚未确认
    pub pending: bool,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TransferLinesPayload {
    pub run: TaxRun,
    pub lines: Vec<TransferLine>,
    pub stats: LineStats,
}

#[derive(Debug, Clone, Deserialize, Type)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum TransferAction {
    /// 设为转出（保留已改税额）
    Include,
    /// 设为不转出
    Exclude,
    /// 设为转出并指定转出税额（分）
    SetTax { tax: i64 },
    /// 复核通过：采纳机器结论（之后不再参与重新匹配）
    AcceptSystem,
    /// 清除人工结论，恢复为机器结论（重新匹配时会再次参与）
    Restore,
    /// 仅修改备注
    SetRemark,
}

#[derive(Debug, Clone, Default, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ConfirmedQuery {
    pub filing_from: Option<String>,
    pub filing_to: Option<String>,
    pub src_from: Option<String>,
    pub src_to: Option<String>,
    pub keyword: Option<String>,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ConfirmedTransferRow {
    pub run_id: i64,
    pub filing_period: PeriodRange,
    pub cert_period: String,
    pub batch_no: i64,
    pub acct_voucher_no: Option<String>,
    pub acct_posted_date: Option<String>,
    pub acct_preparer: Option<String>,
    pub confirmed_by: String,
    pub confirmed_at: String,
    pub detail_id: i64,
    pub header_id: i64,
    pub invoice_code: String,
    pub invoice_no: String,
    pub issue_date: String,
    pub seller_name: String,
    pub seller_tax_no: String,
    pub line_no: i64,
    pub tax_code: Option<String>,
    pub goods_category: Option<String>,
    pub goods_name: String,
    pub amount: i64,
    pub tax_rate: Option<String>,
    pub detail_tax: i64,
    pub transfer_tax: i64,
    pub match_type: MatchType,
    pub hit_rules: Option<String>,
    pub remark: Option<String>,
    pub voucher_nos: Option<String>,
    pub voucher_dates: Option<String>,
}

// ─────────────────────────────────────────────────────────────
// 匹配
// ─────────────────────────────────────────────────────────────

struct Candidate {
    detail_id: i64,
    header_id: i64,
    tax_code: Option<String>,
    goods_category: Option<String>,
    goods_name: String,
    seller_tax_no: String,
    confirmed: bool,
    manual: bool,
    other_run: bool,
}

pub fn run_match(conn: &mut Connection, filing: &PeriodRange, source: &PeriodRange, user: &str) -> Result<MatchSummary> {
    let tx = conn.transaction()?;
    let run = get_or_create_run(&tx, M, filing, source, user)?;
    run.ensure_editable()?;
    let rules = rule_engine::load_rules(&tx, SCOPE)?;

    // 1) 清掉可重算的机器结果（已确认、已人工判定的保留）
    tx.execute(
        "DELETE FROM welfare_transfer_results
         WHERE run_id = ?1
           AND detail_id IN (SELECT d.id FROM invoice_details d JOIN certified_list c ON c.header_id=d.header_id
                             WHERE c.tax_period BETWEEN ?2 AND ?3)
           AND detail_id NOT IN (SELECT detail_id FROM welfare_transfer_confirmed_lines)
           AND detail_id NOT IN (SELECT detail_id FROM welfare_transfer_overrides
                                 WHERE run_id = ?1 AND (included IS NOT NULL OR transfer_tax IS NOT NULL))",
        params![run.id, source.from, source.to],
    )?;

    // 2) 候选明细
    let cands: Vec<Candidate> = {
        let mut stmt = tx.prepare(
            "SELECT d.id, d.header_id, d.tax_code, d.goods_category, d.goods_name, s.tax_no,
                    EXISTS (SELECT 1 FROM welfare_transfer_confirmed_lines x WHERE x.detail_id = d.id),
                    EXISTS (SELECT 1 FROM welfare_transfer_overrides x WHERE x.detail_id = d.id AND x.run_id = ?1
                            AND (x.included IS NOT NULL OR x.transfer_tax IS NOT NULL)),
                    EXISTS (SELECT 1 FROM welfare_transfer_results x WHERE x.detail_id = d.id)
                    OR EXISTS (SELECT 1 FROM welfare_transfer_overrides x WHERE x.detail_id = d.id AND x.run_id <> ?1 AND x.included = 1)
             FROM certified_list c
             JOIN invoice_headers h ON h.id = c.header_id
             JOIN sellers s         ON s.id = h.seller_id
             JOIN invoice_details d ON d.header_id = h.id
             WHERE c.tax_period BETWEEN ?2 AND ?3
               AND EXISTS (SELECT 1 FROM v_invoice_vouchers v WHERE v.header_id = h.id)",
        )?;
        let rows = stmt
            .query_map(params![run.id, source.from, source.to], |r| {
                Ok(Candidate {
                    detail_id: r.get(0)?,
                    header_id: r.get(1)?,
                    tax_code: r.get(2)?,
                    goods_category: r.get(3)?,
                    goods_name: r.get(4)?,
                    seller_tax_no: r.get(5)?,
                    confirmed: r.get(6)?,
                    manual: r.get(7)?,
                    other_run: r.get(8)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        rows
    };

    // 3) 逐条规则评估
    let ts = now();
    let (mut hit_count, mut sk_manual, mut sk_conf, mut sk_other) = (0i64, 0i64, 0i64, 0i64);
    {
        let mut ins_res = tx.prepare_cached(
            "INSERT INTO welfare_transfer_results (run_id, detail_id, header_id, matched_at) VALUES (?1, ?2, ?3, ?4)",
        )?;
        let mut ins_hit =
            tx.prepare_cached("INSERT INTO welfare_transfer_rule_hits (result_id, rule_id) VALUES (?1, ?2)")?;
        for c in &cands {
            if c.confirmed {
                sk_conf += 1;
                continue;
            }
            if c.other_run {
                sk_other += 1;
                continue;
            }
            if c.manual {
                sk_manual += 1;
                continue;
            }
            let input = RuleInput {
                seller_tax_no: &c.seller_tax_no,
                tax_code: c.tax_code.as_deref(),
                goods_category: c.goods_category.as_deref(),
                goods_name: &c.goods_name,
            };
            let hits = rule_engine::hits(&rules, &input);
            if hits.is_empty() {
                continue;
            }
            ins_res.execute(params![run.id, c.detail_id, c.header_id, ts])?;
            let rid = tx.last_insert_rowid();
            for h in hits {
                ins_hit.execute(params![rid, h.id])?;
            }
            hit_count += 1;
        }
    }

    touch_run(&tx, run.id, true)?;
    log_event(
        &tx,
        run.id,
        "matched",
        &format!(
            "纳税所属期{} 来源范围{}；规则{} 候选{} 命中{} 跳过(人工{} 已确认{} 其它所属期{})",
            filing.label(),
            source.label(),
            rules.len(),
            cands.len(),
            hit_count,
            sk_manual,
            sk_conf,
            sk_other
        ),
        user,
    )?;
    let run = load_run(&tx, run.id)?;
    tx.commit()?;
    Ok(MatchSummary {
        run,
        rule_count: rules.len() as i64,
        candidate_count: cands.len() as i64,
        hit_count,
        skipped_manual: sk_manual,
        skipped_confirmed: sk_conf,
        skipped_other_run: sk_other,
    })
}

// ─────────────────────────────────────────────────────────────
// 列表（复核）
// ─────────────────────────────────────────────────────────────

fn rule_categories(conn: &Connection) -> Result<HashMap<i64, String>> {
    let mut stmt = conn.prepare_cached("SELECT id, rule_category FROM tax_match_rules WHERE scope = ?1")?;
    let m = stmt
        .query_map([SCOPE], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?
        .collect::<rusqlite::Result<HashMap<_, _>>>()?;
    Ok(m)
}

fn parse_ids(s: Option<String>) -> Vec<i64> {
    s.map(|x| x.split(',').filter_map(|t| t.trim().parse().ok()).collect()).unwrap_or_default()
}

pub fn list_lines(conn: &Connection, run_id: i64) -> Result<TransferLinesPayload> {
    let run = load_run(conn, run_id)?;
    run.ensure_module(M)?;
    let cats = rule_categories(conn)?;

    let sql = format!(
        "SELECT r.id AS result_id, r.run_id AS result_run_id, r.matched_at,
                d.id AS detail_id, d.header_id, c.tax_period AS cert_period,
                COALESCE(trim(h.code), '') AS invoice_code, {INVOICE_NO_SQL} AS invoice_no,
                h.issue_date, s.name AS seller_name, s.tax_no AS seller_tax_no,
                d.line_no, d.tax_code, d.goods_category, d.goods_name, d.spec_model,
                d.amount, d.tax_rate, d.tax,
                (SELECT group_concat(DISTINCT v.source || ' ' || v.voucher_no)
                   FROM v_invoice_vouchers v WHERE v.header_id = h.id) AS voucher_nos,
                (SELECT group_concat(DISTINCT v.posted_date)
                   FROM v_invoice_vouchers v WHERE v.header_id = h.id) AS voucher_dates,
                (SELECT group_concat(rh.rule_id)
                   FROM welfare_transfer_rule_hits rh WHERE rh.result_id = r.id) AS hit_rule_ids,
                (SELECT group_concat(t.rule_category || CASE WHEN t.note <> '' THEN '（' || t.note || '）' ELSE '' END, '；')
                   FROM welfare_transfer_rule_hits rh JOIN tax_match_rules t ON t.id = rh.rule_id
                  WHERE rh.result_id = r.id) AS hit_rules,
                (SELECT MAX(t.needs_manual_review)
                   FROM welfare_transfer_rule_hits rh JOIN tax_match_rules t ON t.id = rh.rule_id
                  WHERE rh.result_id = r.id) AS hit_needs_review,
                o.id AS ov_id, o.included AS ov_included, o.transfer_tax AS ov_tax, o.remark AS ov_remark,
                o.updated_by AS ov_by, o.updated_at AS ov_at,
                cl.transfer_tax AS cl_tax, cl.decision AS cl_decision, b.batch_no AS cl_batch_no
         FROM certified_list c
         JOIN invoice_headers h ON h.id = c.header_id
         JOIN sellers s         ON s.id = h.seller_id
         JOIN invoice_details d ON d.header_id = h.id
         LEFT JOIN welfare_transfer_results r          ON r.detail_id  = d.id
         LEFT JOIN welfare_transfer_overrides o        ON o.detail_id  = d.id AND o.run_id = ?1
         LEFT JOIN welfare_transfer_confirmed_lines cl ON cl.detail_id = d.id
         LEFT JOIN tax_run_batches b                   ON b.id = cl.batch_id
         WHERE EXISTS (SELECT 1 FROM tax_run_sources rs WHERE rs.run_id = ?1
                       AND c.tax_period BETWEEN rs.src_period_from AND rs.src_period_to)
         ORDER BY c.tax_period, h.issue_date, h.id, d.line_no"
    );
    let mut stmt = conn.prepare(&sql)?;
    let lines = stmt
        .query_map(params![run.id], |r| {
            let result_id: Option<i64> = r.get("result_id")?;
            let tax: i64 = r.get("tax")?;
            let ov_included: Option<i64> = r.get("ov_included")?;
            let ov_tax: Option<i64> = r.get("ov_tax")?;
            let cl_tax: Option<i64> = r.get("cl_tax")?;
            let cl_decision: Option<String> = r.get("cl_decision")?;
            let hit_ids = parse_ids(r.get("hit_rule_ids")?);
            let voucher_nos: Option<String> = r.get("voucher_nos")?;
            let hit_review: Option<i64> = r.get("hit_needs_review")?;
            let result_run: Option<i64> = r.get("result_run_id")?;
            let foreign_hit = result_id.is_some() && result_run != Some(run.id);
            let auto_hit = result_id.is_some() && !foreign_hit;
            let confirmed = cl_tax.is_some();

            let (final_included, final_tax, match_type) = if let Some(t) = cl_tax {
                (true, t, MatchType::from_decision(cl_decision.as_deref().unwrap_or("auto")))
            } else {
                derive_final(auto_hit, tax, ov_included, ov_tax)
            };
            let needs_review =
                !confirmed && auto_hit && hit_review == Some(1) && ov_included.is_none() && ov_tax.is_none();

            let mut hit_categories: Vec<String> = Vec::new();
            for id in &hit_ids {
                if let Some(c) = cats.get(id) {
                    if !hit_categories.contains(c) {
                        hit_categories.push(c.clone());
                    }
                }
            }

            Ok(TransferLine {
                result_id,
                matched_at: r.get("matched_at")?,
                detail_id: r.get("detail_id")?,
                header_id: r.get("header_id")?,
                cert_period: r.get("cert_period")?,
                invoice_code: r.get("invoice_code")?,
                invoice_no: r.get("invoice_no")?,
                issue_date: r.get("issue_date")?,
                seller_name: r.get("seller_name")?,
                seller_tax_no: r.get("seller_tax_no")?,
                line_no: r.get("line_no")?,
                tax_code: r.get("tax_code")?,
                goods_category: r.get("goods_category")?,
                goods_name: r.get("goods_name")?,
                spec_model: r.get("spec_model")?,
                amount: r.get("amount")?,
                tax_rate: r.get("tax_rate")?,
                tax,
                has_voucher: voucher_nos.is_some(),
                voucher_nos,
                voucher_dates: r.get("voucher_dates")?,
                auto_hit,
                matched_in_other_period: foreign_hit,
                hit_rule_ids: hit_ids,
                hit_categories,
                hit_rules: r.get("hit_rules")?,
                override_included: ov_included.map(|v| v == 1),
                override_tax: ov_tax,
                remark: r.get("ov_remark")?,
                updated_by: r.get("ov_by")?,
                updated_at: r.get("ov_at")?,
                final_included,
                final_tax,
                match_type,
                needs_review,
                confirmed_batch_no: r.get("cl_batch_no")?,
                locked: confirmed,
                pending: final_included && !confirmed,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let stats = compute_stats(conn, &run, &lines)?;
    Ok(TransferLinesPayload { run, lines, stats })
}

/// (是否转出, 转出税额, 匹配类型)
fn derive_final(auto_hit: bool, detail_tax: i64, ov_included: Option<i64>, ov_tax: Option<i64>) -> (bool, i64, MatchType) {
    let included = ov_included.map(|v| v == 1).unwrap_or(auto_hit);
    let tax = if included { ov_tax.unwrap_or(detail_tax) } else { 0 };
    let manual = ov_included.map_or(false, |v| (v == 1) != auto_hit)
        || (included && ov_tax.map_or(false, |t| t != detail_tax));
    let mt = if manual {
        MatchType::Manual
    } else if ov_included.is_some() || ov_tax.is_some() {
        MatchType::Reviewed
    } else if auto_hit {
        MatchType::AutoHit
    } else {
        MatchType::NoHit
    };
    (included, tax, mt)
}

fn snapshot(pending: &[&TransferLine]) -> PendingSnapshot {
    PendingSnapshot {
        fingerprint: fingerprint(pending.iter().map(|l| format!("{}:{}", l.detail_id, l.final_tax))),
        line_count: pending.len() as i64,
        tax_total: pending.iter().map(|l| l.final_tax).sum(),
    }
}

fn compute_stats(conn: &Connection, run: &TaxRun, lines: &[TransferLine]) -> Result<LineStats> {
    let mut s = LineStats { total_lines: lines.len() as i64, ..Default::default() };
    for l in lines {
        if l.auto_hit {
            s.auto_hit_count += 1;
        }
        if l.final_included {
            s.included_count += 1;
            s.included_tax += l.final_tax;
        }
        if l.pending {
            s.pending_count += 1;
            s.pending_tax += l.final_tax;
        }
        if l.needs_review {
            s.needs_review_count += 1;
        }
        if l.match_type == MatchType::Manual {
            s.manual_count += 1;
        }
        if l.locked {
            s.confirmed_count += 1;
            s.confirmed_tax += l.final_tax;
        }
    }
    if let Some(b) = run.pending_batch() {
        if let Some(fp) = batch_export_fingerprint(conn, b.id)? {
            s.exported = true;
            let pending: Vec<&TransferLine> = lines.iter().filter(|l| l.pending).collect();
            s.export_stale = fp != snapshot(&pending).fingerprint;
        }
    }
    Ok(s)
}

// ─────────────────────────────────────────────────────────────
// 人工修改
// ─────────────────────────────────────────────────────────────

pub fn apply_override(
    conn: &mut Connection,
    run_id: i64,
    detail_ids: &[i64],
    action: &TransferAction,
    remark: Option<String>,
    user: &str,
) -> Result<ApplyResult> {
    let tx = conn.transaction()?;
    let run = load_run(&tx, run_id)?;
    run.ensure_module(M)?;
    run.ensure_editable()?;
    let remark = remark.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    let ts = now();
    let (mut applied, mut skipped) = (0i64, 0i64);
    {
        let mut info = tx.prepare_cached(
            "SELECT d.tax,
                    EXISTS (SELECT 1 FROM welfare_transfer_results x WHERE x.detail_id = d.id AND x.run_id = ?2),
                    EXISTS (SELECT 1 FROM welfare_transfer_confirmed_lines x WHERE x.detail_id = d.id),
                    EXISTS (SELECT 1 FROM welfare_transfer_results x WHERE x.detail_id = d.id AND x.run_id <> ?2)
             FROM invoice_details d
             WHERE d.id = ?1 AND EXISTS (
                 SELECT 1 FROM certified_list c JOIN tax_run_sources rs
                   ON rs.run_id = ?2 AND c.tax_period BETWEEN rs.src_period_from AND rs.src_period_to
                 WHERE c.header_id = d.header_id)",
        )?;
        let mut existing = tx.prepare_cached(
            "SELECT included, transfer_tax, remark FROM welfare_transfer_overrides WHERE detail_id = ?1 AND run_id = ?2",
        )?;
        let mut upsert = tx.prepare_cached(
            "INSERT INTO welfare_transfer_overrides
                 (run_id, detail_id, included, transfer_tax, remark, updated_by, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(run_id, detail_id) DO UPDATE SET
                  included = excluded.included, transfer_tax = excluded.transfer_tax,
                 remark = excluded.remark, updated_by = excluded.updated_by, updated_at = excluded.updated_at",
        )?;
        let mut del = tx.prepare_cached("DELETE FROM welfare_transfer_overrides WHERE detail_id = ?1 AND run_id = ?2")?;

        for &id in detail_ids {
            let row: Option<(i64, bool, bool, bool)> = info
                .query_row(params![id, run.id], |r| {
                    Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
                })
                .optional()?;
            let (dtax, auto_hit, confirmed, other) =
                row.ok_or_else(|| anyhow!("明细 {id} 不属于当前纳税所属期已选认证期间"))?;
            if confirmed {
                skipped += 1; // 已确认转出：只增不减，不允许任何修改
                continue;
            }
            let (mut inc, mut tax, mut rem): (Option<i64>, Option<i64>, Option<String>) = existing
                .query_row(params![id, run.id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
                .optional()?
                .unwrap_or((None, None, None));

            match action {
                TransferAction::Restore => {
                    del.execute(params![id, run.id])?;
                    applied += 1;
                    continue;
                }
                TransferAction::Include => inc = Some(1),
                TransferAction::Exclude => {
                    inc = Some(0);
                    tax = None;
                }
                TransferAction::SetTax { tax: t } => {
                    if dtax < 0 {
                        bail!("明细 {id} 为负数税额（红字），不支持修改转出税额");
                    }
                    if *t < 0 || *t > dtax {
                        bail!("明细 {id} 转出税额须在 0 ~ {:.2} 元之间", fen_to_yuan(dtax));
                    }
                    if *t == 0 {
                        inc = Some(0);
                        tax = None;
                    } else {
                        inc = Some(1);
                        tax = if *t == dtax { None } else { Some(*t) };
                    }
                }
                TransferAction::AcceptSystem => {
                    inc = Some(auto_hit as i64);
                    tax = None;
                }
                TransferAction::SetRemark => rem = remark.clone(),
            }
            if !matches!(action, TransferAction::SetRemark) && remark.is_some() {
                rem = remark.clone();
            }
            if inc.is_none() && tax.is_none() && rem.is_none() {
                del.execute([id])?;
            } else {
                upsert.execute(params![run.id, id, inc, tax, rem, user, ts])?;
            }
            applied += 1;
        }
    }
    touch_run(&tx, run.id, false)?;
    tx.commit()?;
    Ok(ApplyResult { applied, skipped_locked: skipped })
}

// ─────────────────────────────────────────────────────────────
// 导出
// ─────────────────────────────────────────────────────────────

const LINE_COLS: &[Col] = &[
    col("结果ID", 8.0),
    col("明细ID", 8.0),
    col("发票头ID", 9.0),
    col("认证期间", 9.0),
    col("匹配时间", 18.0),
    col("发票代码", 13.0),
    col("发票号码", 22.0),
    col("开票日期", 11.0),
    col("销售方名称", 30.0),
    col("销售方税号", 21.0),
    col("行号", 6.0),
    col("税收分类编码", 21.0),
    col("税收分类", 14.0),
    col("货物或应税劳务名称", 28.0),
    col("规格型号", 12.0),
    col("金额", 13.0),
    col("税率", 7.0),
    col("税额", 12.0),
    col("转出税额", 12.0),
    col("匹配类型", 14.0),
    col("命中规则", 30.0),
    col("原记账凭证(来源 凭证号)", 26.0),
    col("原记账日期", 22.0),
    col("备注", 24.0),
    col("确认批次", 8.0),
];

fn line_row(l: &TransferLine) -> Vec<Val> {
    vec![
        l.result_id.map(Val::Int).unwrap_or(Val::Empty),
        Val::Int(l.detail_id),
        Val::Int(l.header_id),
        l.cert_period.clone().into(),
        l.matched_at.clone().into(),
        l.invoice_code.clone().into(),
        l.invoice_no.clone().into(),
        l.issue_date.clone().into(),
        l.seller_name.clone().into(),
        l.seller_tax_no.clone().into(),
        Val::Int(l.line_no),
        l.tax_code.clone().into(),
        l.goods_category.clone().into(),
        l.goods_name.clone().into(),
        l.spec_model.clone().into(),
        Val::Money(l.amount),
        l.tax_rate.clone().into(),
        Val::Money(l.tax),
        Val::Money(l.final_tax),
        l.match_type.label().into(),
        l.hit_rules.clone().into(),
        l.voucher_nos.clone().into(),
        l.voucher_dates.clone().into(),
        l.remark.clone().into(),
        l.confirmed_batch_no.map(Val::Int).unwrap_or(Val::Empty),
    ]
}

fn write_xlsx(path: &str, run: &TaxRun, batch: &RunBatch, lines: &[&TransferLine], user: &str) -> Result<()> {
    let mut wb = Workbook::new();
    let rows: Vec<Vec<Val>> = lines.iter().map(|l| line_row(l)).collect();
    write_table(&mut wb, "转出明细", LINE_COLS, &rows)?;

    // 按规则类别汇总
    let mut by_cat: Vec<(String, i64, i64)> = Vec::new();
    for l in lines {
        let key = match l.match_type {
            MatchType::Manual if l.hit_categories.is_empty() => "人工指定".to_string(),
            _ => l.hit_categories.first().cloned().unwrap_or_else(|| "人工指定".into()),
        };
        match by_cat.iter_mut().find(|x| x.0 == key) {
            Some(x) => {
                x.1 += 1;
                x.2 += l.final_tax;
            }
            None => by_cat.push((key, 1, l.final_tax)),
        }
    }
    let total: i64 = lines.iter().map(|l| l.final_tax).sum();
    let info = vec![
        ("模块", M.label().to_string()),
        ("认证期间范围", run.src_periods.iter().map(PeriodRange::label).collect::<Vec<_>>().join("；")),
        ("纳税所属期", run.filing_period.as_ref().map(|p| p.label()).unwrap_or_default()),
        ("批次", format!("{}（{}）", batch.batch_no, if batch.batch_no == 1 { "首次" } else { "补充/补税" })),
        ("明细行数", lines.len().to_string()),
        ("转出税额合计(元)", format!("{:.2}", fen_to_yuan(total))),
        ("会计凭证号", batch.acct_voucher_no.clone().unwrap_or_default()),
        ("入账日期", batch.acct_posted_date.clone().unwrap_or_default()),
        ("做账人", batch.acct_preparer.clone().unwrap_or_default()),
        ("导出人", user.to_string()),
        ("导出时间", now()),
    ];
    let sum_cols = [col("规则类别", 22.0), col("行数", 10.0), col("转出税额", 14.0)];
    let sum_rows: Vec<Vec<Val>> =
        by_cat.into_iter().map(|(k, n, t)| vec![Val::Text(k), Val::Int(n), Val::Money(t)]).collect();
    write_info_sheet(&mut wb, "说明及汇总", &info, &sum_cols, &sum_rows)?;
    wb.save(path)?;
    Ok(())
}

/// 导出本批待确认清单（交会计做账），并记录导出指纹
pub fn export_pending_batch(conn: &Connection, run_id: i64, path: &str, user: &str) -> Result<ExportResult> {
    let p = list_lines(conn, run_id)?;
    p.run.ensure_editable()?;
    if p.stats.needs_review_count > 0 {
        bail!("还有 {} 条命中“需人工核对”规则的明细未复核，请先处理", p.stats.needs_review_count);
    }
    let batch = p.run.pending_batch().cloned().ok_or_else(|| anyhow!("没有待确认的批次"))?;
    let pending: Vec<&TransferLine> = p.lines.iter().filter(|l| l.pending).collect();
    if batch.batch_no > 1 && pending.is_empty() {
        bail!("补充批次没有新增转出明细");
    }
    write_xlsx(path, &p.run, &batch, &pending, user)?;
    let snap = snapshot(&pending);
    record_export(conn, &p.run, &snap, path, user)?;
    Ok(ExportResult { path: path.into(), line_count: snap.line_count, tax_total: snap.tax_total })
}

/// 重新导出某个已确认批次（含会计凭证信息）
pub fn export_confirmed_batch(conn: &Connection, run_id: i64, batch_no: i64, path: &str, user: &str) -> Result<ExportResult> {
    let p = list_lines(conn, run_id)?;
    let batch = p
        .run
        .batches
        .iter()
        .find(|b| b.batch_no == batch_no && b.confirmed_at.is_some())
        .cloned()
        .ok_or_else(|| anyhow!("批次 {batch_no} 不存在或尚未确认"))?;
    let lines: Vec<&TransferLine> = p.lines.iter().filter(|l| l.confirmed_batch_no == Some(batch_no)).collect();
    write_xlsx(path, &p.run, &batch, &lines, user)?;
    Ok(ExportResult {
        path: path.into(),
        line_count: lines.len() as i64,
        tax_total: lines.iter().map(|l| l.final_tax).sum(),
    })
}

// ─────────────────────────────────────────────────────────────
// 确认
// ─────────────────────────────────────────────────────────────

pub fn confirm(conn: &mut Connection, input: &ConfirmInput, user: &str) -> Result<TaxRun> {
    let tx = conn.transaction()?;
    let p = list_lines(&tx, input.run_id)?;
    if p.stats.needs_review_count > 0 {
        bail!("还有 {} 条明细待复核", p.stats.needs_review_count);
    }
    let pending: Vec<&TransferLine> = p.lines.iter().filter(|l| l.pending).collect();
    let snap = snapshot(&pending);
    let (run, batch, voucher) = begin_confirm(&tx, M, input, &snap)?;
    let ts = now();
    {
        let mut ins = tx.prepare_cached(
            "INSERT INTO welfare_transfer_confirmed_lines
                 (run_id, batch_id, detail_id, header_id, cert_period, detail_tax, transfer_tax, decision, remark, confirmed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        )?;
        for l in &pending {
            ins.execute(params![
                run.id,
                batch.id,
                l.detail_id,
                l.header_id,
                l.cert_period,
                l.tax,
                l.final_tax,
                l.match_type.decision_str(),
                l.remark,
                ts
            ])?;
        }
    }
    finish_confirm(&tx, &run, &batch, &voucher, &snap, &ts, user)?;
    let run = load_run(&tx, run.id)?;
    tx.commit()?;
    Ok(run)
}

/// 已解锁期间：本批待确认行数（重新锁定前校验）
pub fn pending_count(conn: &Connection, run_id: i64) -> Result<i64> {
    Ok(list_lines(conn, run_id)?.stats.pending_count)
}

pub fn delete_run_data(conn: &Connection, run_id: i64) -> Result<()> {
    conn.execute("DELETE FROM welfare_transfer_overrides WHERE run_id = ?1", [run_id])?;
    conn.execute("DELETE FROM welfare_transfer_results WHERE run_id = ?1", [run_id])?; // hits 级联
    Ok(())
}

// ─────────────────────────────────────────────────────────────
// 已确认明细查询（任意纳税所属期）
// ─────────────────────────────────────────────────────────────

pub fn query_confirmed(conn: &Connection, q: &ConfirmedQuery) -> Result<Vec<ConfirmedTransferRow>> {
    let kw = q
        .keyword
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| format!("%{}%", s.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_")));
    let sql = format!(
        "SELECT tr.id AS run_id, tr.filing_period_from, tr.filing_period_to, cl.cert_period,
                b.batch_no, b.acct_voucher_no, b.acct_posted_date, b.acct_preparer, b.confirmed_by, b.confirmed_at,
                cl.detail_id, cl.header_id, COALESCE(trim(h.code), '') AS invoice_code, {INVOICE_NO_SQL} AS invoice_no,
                h.issue_date, s.name AS seller_name, s.tax_no AS seller_tax_no,
                d.line_no, d.tax_code, d.goods_category, d.goods_name, d.amount, d.tax_rate,
                cl.detail_tax, cl.transfer_tax, cl.decision, cl.remark,
                (SELECT group_concat(t.rule_category || CASE WHEN t.note <> '' THEN '（' || t.note || '）' ELSE '' END, '；')
                   FROM welfare_transfer_results r
                   JOIN welfare_transfer_rule_hits rh ON rh.result_id = r.id
                   JOIN tax_match_rules t ON t.id = rh.rule_id
                  WHERE r.detail_id = cl.detail_id) AS hit_rules,
                (SELECT group_concat(DISTINCT v.source || ' ' || v.voucher_no)
                   FROM v_invoice_vouchers v WHERE v.header_id = cl.header_id) AS voucher_nos,
                (SELECT group_concat(DISTINCT v.posted_date)
                   FROM v_invoice_vouchers v WHERE v.header_id = cl.header_id) AS voucher_dates
         FROM welfare_transfer_confirmed_lines cl
         JOIN tax_runs tr        ON tr.id = cl.run_id
         JOIN tax_run_batches b  ON b.id  = cl.batch_id
         JOIN invoice_details d  ON d.id  = cl.detail_id
         JOIN invoice_headers h  ON h.id  = cl.header_id
         JOIN sellers s          ON s.id  = h.seller_id
         WHERE (?1 IS NULL OR tr.filing_period_to   >= ?1)
           AND (?2 IS NULL OR tr.filing_period_from <= ?2)
           AND (?3 IS NULL OR cl.cert_period >= ?3)
           AND (?4 IS NULL OR cl.cert_period <= ?4)
           AND (?5 IS NULL OR h.inv_unq_key LIKE ?5 {LIKE_ESCAPE} OR s.name LIKE ?5 {LIKE_ESCAPE}
                OR s.tax_no LIKE ?5 {LIKE_ESCAPE} OR d.goods_name LIKE ?5 {LIKE_ESCAPE}
                OR b.acct_voucher_no LIKE ?5 {LIKE_ESCAPE})
         ORDER BY tr.filing_period_from, b.batch_no, h.issue_date, h.id, d.line_no"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map(params![q.filing_from, q.filing_to, q.src_from, q.src_to, kw], |r| {
            Ok(ConfirmedTransferRow {
                run_id: r.get("run_id")?,
                filing_period: PeriodRange { from: r.get("filing_period_from")?, to: r.get("filing_period_to")? },
                cert_period: r.get("cert_period")?,
                batch_no: r.get("batch_no")?,
                acct_voucher_no: r.get("acct_voucher_no")?,
                acct_posted_date: r.get("acct_posted_date")?,
                acct_preparer: r.get("acct_preparer")?,
                confirmed_by: r.get("confirmed_by")?,
                confirmed_at: r.get("confirmed_at")?,
                detail_id: r.get("detail_id")?,
                header_id: r.get("header_id")?,
                invoice_code: r.get("invoice_code")?,
                invoice_no: r.get("invoice_no")?,
                issue_date: r.get("issue_date")?,
                seller_name: r.get("seller_name")?,
                seller_tax_no: r.get("seller_tax_no")?,
                line_no: r.get("line_no")?,
                tax_code: r.get("tax_code")?,
                goods_category: r.get("goods_category")?,
                goods_name: r.get("goods_name")?,
                amount: r.get("amount")?,
                tax_rate: r.get("tax_rate")?,
                detail_tax: r.get("detail_tax")?,
                transfer_tax: r.get("transfer_tax")?,
                match_type: MatchType::from_decision(&r.get::<_, String>("decision")?),
                hit_rules: r.get("hit_rules")?,
                remark: r.get("remark")?,
                voucher_nos: r.get("voucher_nos")?,
                voucher_dates: r.get("voucher_dates")?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn export_confirmed_query(conn: &Connection, q: &ConfirmedQuery, path: &str) -> Result<ExportResult> {
    let rows = query_confirmed(conn, q)?;
    let cols = [
        col("纳税所属期", 14.0),
        col("认证期间", 9.0),
        col("批次", 6.0),
        col("会计凭证号", 14.0),
        col("入账日期", 11.0),
        col("做账人", 10.0),
        col("发票代码", 13.0),
        col("发票号码", 22.0),
        col("开票日期", 11.0),
        col("销售方名称", 30.0),
        col("销售方税号", 21.0),
        col("行号", 6.0),
        col("税收分类编码", 21.0),
        col("税收分类", 14.0),
        col("货物或应税劳务名称", 28.0),
        col("金额", 13.0),
        col("税率", 7.0),
        col("税额", 12.0),
        col("转出税额", 12.0),
        col("匹配类型", 14.0),
        col("命中规则", 30.0),
        col("原记账凭证", 26.0),
        col("原记账日期", 22.0),
        col("备注", 24.0),
        col("确认人", 10.0),
        col("确认时间", 18.0),
    ];
    let data: Vec<Vec<Val>> = rows
        .iter()
        .map(|r| {
            vec![
                r.filing_period.label().into(),
                r.cert_period.clone().into(),
                Val::Int(r.batch_no),
                r.acct_voucher_no.clone().into(),
                r.acct_posted_date.clone().into(),
                r.acct_preparer.clone().into(),
                r.invoice_code.clone().into(),
                r.invoice_no.clone().into(),
                r.issue_date.clone().into(),
                r.seller_name.clone().into(),
                r.seller_tax_no.clone().into(),
                Val::Int(r.line_no),
                r.tax_code.clone().into(),
                r.goods_category.clone().into(),
                r.goods_name.clone().into(),
                Val::Money(r.amount),
                r.tax_rate.clone().into(),
                Val::Money(r.detail_tax),
                Val::Money(r.transfer_tax),
                r.match_type.label().into(),
                r.hit_rules.clone().into(),
                r.voucher_nos.clone().into(),
                r.voucher_dates.clone().into(),
                r.remark.clone().into(),
                r.confirmed_by.clone().into(),
                r.confirmed_at.clone().into(),
            ]
        })
        .collect();
    let mut wb = Workbook::new();
    write_table(&mut wb, "已确认进项转出", &cols, &data)?;
    wb.save(path)?;
    Ok(ExportResult {
        path: path.into(),
        line_count: rows.len() as i64,
        tax_total: rows.iter().map(|r| r.transfer_tax).sum(),
    })
}
