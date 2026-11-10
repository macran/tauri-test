//! 印花税
//!
//! 数据范围：发票在 AP / IC / 银行 / 差旅 任一凭证中、凭证入账期间 ∈ 凭证期间范围
//!           同一发票可能在多个期间都有凭证：先被哪个所属期批次匹配/人工判定，就归属哪个
//! 匹配：scope = 'stamp_tax' 的启用规则；每条规则带 stamp_tax_category_code
//! 类别：命中规则的类别唯一 → 自动采用；多个不同类别 → 需人工指定
//! 计税金额 = 明细不含税金额；税额 = 计税金额 × rate_ppm / 1e6（四舍五入到分）
//! 采集表：按子目(或父级税目)汇总计税金额，按汇总金额 × 税率计算税额

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use anyhow::{anyhow, bail, Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use rust_xlsxwriter::Workbook;
use serde::{Deserialize, Serialize};
use specta::Type;

use super::common::*;
use super::excel::{col, write_info_sheet, write_table, Col, Val};
use super::rule_engine::{self, RuleInput};
use super::xlsx_template::{self, set_cell, CellMap, CellValue};
use super::{INVOICE_NO_SQL, LIKE_ESCAPE};

const M: TaxModule = TaxModule::StampTax;
const SCOPE: &str = "stamp_tax";
const TEMPLATE_KEY: &str = "stamp_tax.collection_template";

// ─────────────────────────────────────────────────────────────
// 印花税类别
// ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct StampCategory {
    pub code: String,
    pub name: String,
    pub parent_code: Option<String>,
    pub rate_ppm: Option<i64>,
}

pub fn list_categories(conn: &Connection) -> Result<Vec<StampCategory>> {
    let mut stmt = conn.prepare("SELECT code, name, parent_code, rate_ppm FROM stamp_tax_categories ORDER BY code")?;
    let rows = stmt
        .query_map([], |r| {
            Ok(StampCategory { code: r.get(0)?, name: r.get(1)?, parent_code: r.get(2)?, rate_ppm: r.get(3)? })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

fn category_map(conn: &Connection) -> Result<HashMap<String, StampCategory>> {
    Ok(list_categories(conn)?.into_iter().map(|c| (c.code.clone(), c)).collect())
}

/// (税目代码, 税目名称, 子目代码, 子目名称)
fn item_and_sub(cats: &HashMap<String, StampCategory>, code: &str) -> (String, String, Option<String>, Option<String>) {
    match cats.get(code) {
        Some(c) => match c.parent_code.as_ref().and_then(|p| cats.get(p)) {
            Some(p) => (p.code.clone(), p.name.clone(), Some(c.code.clone()), Some(c.name.clone())),
            None => (c.code.clone(), c.name.clone(), None, None),
        },
        None => (code.to_string(), code.to_string(), None, None),
    }
}

pub fn calc_tax(base_fen: i64, rate_ppm: i64) -> i64 {
    let p = base_fen as i128 * rate_ppm as i128;
    let q = (p.abs() + 500_000) / 1_000_000;
    (if p < 0 { -q } else { q }) as i64
}

// ─────────────────────────────────────────────────────────────
// 类型
// ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct StampLine {
    pub result_id: Option<i64>,
    pub matched_at: Option<String>,
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
    pub spec_model: Option<String>,
    pub amount: i64,
    pub tax_rate: Option<String>,
    pub vat_tax: i64,
    /// 已选凭证期间范围内的凭证
    pub voucher_nos: Option<String>,
    pub voucher_dates: Option<String>,
    pub po_nos: Option<String>,
    pub cost_centers: Option<String>,
    pub auto_hit: bool,
    /// 系统已在其他纳税所属期命中过；当前批次不重复跑规则，但仍可人工复核。
    pub matched_in_other_period: bool,
    pub hit_rule_ids: Vec<i64>,
    pub hit_rules: Option<String>,
    pub hit_categories: Vec<String>,
    pub auto_category: Option<String>,
    pub override_included: Option<bool>,
    pub override_category: Option<String>,
    pub remark: Option<String>,
    pub updated_by: Option<String>,
    pub updated_at: Option<String>,
    pub final_included: bool,
    pub final_category: Option<String>,
    pub item_name: Option<String>,
    pub sub_item_name: Option<String>,
    pub rate_ppm: Option<i64>,
    pub base_amount: i64,
    pub stamp_tax: i64,
    pub match_type: MatchType,
    pub needs_review: bool,
    /// 计入但类别缺失 / 类别为必须选子目的父级
    pub category_invalid: bool,
    pub confirmed_batch_no: Option<i64>,
    pub locked: bool,
    pub pending: bool,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct StampLinesPayload {
    pub run: TaxRun,
    pub lines: Vec<StampLine>,
    pub stats: LineStats,
}

#[derive(Debug, Clone, Deserialize, Type)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum StampAction {
    /// 设为计入（保留已指定类别）
    Include,
    Exclude,
    /// 设为计入并指定类别（子目）
    SetCategory {
        #[serde(rename = "categoryCode")]
        category_code: String,
    },
    AcceptSystem,
    Restore,
    SetRemark,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum GroupLevel {
    /// 按子目汇总
    Child,
    /// 按父级税目汇总
    Parent,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct StampSummaryRow {
    pub item_code: String,
    pub item_name: String,
    pub sub_item_code: Option<String>,
    pub sub_item_name: Option<String>,
    pub rate_ppm: Option<i64>,
    pub line_count: i64,
    pub base_amount: i64,
    pub tax_amount: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TemplateColumns {
    pub seq: Option<String>,
    pub item_code: Option<String>,
    pub item_name: Option<String>,
    pub sub_item_code: Option<String>,
    pub sub_item_name: Option<String>,
    pub period_from: Option<String>,
    pub period_to: Option<String>,
    pub base_amount: Option<String>,
    pub rate: Option<String>,
    pub tax_amount: Option<String>,
    pub line_count: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FixedCell {
    /// 单元格地址，如 B3
    pub cell: String,
    /// 支持占位符：{filingFrom} {filingTo} {periodStart} {periodEnd} {totalBase} {totalTax} {today} {user}
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct StampTemplateSettings {
    pub template_path: Option<String>,
    /// 为空 = 第一个工作表
    pub sheet_name: Option<String>,
    /// 数据起始行（1-based）
    pub start_row: i64,
    pub group_level: GroupLevel,
    pub columns: TemplateColumns,
    pub fixed_cells: Vec<FixedCell>,
}

impl Default for StampTemplateSettings {
    fn default() -> Self {
        Self {
            template_path: None,
            sheet_name: None,
            start_row: 2,
            group_level: GroupLevel::Child,
            columns: TemplateColumns {
                seq: Some("A".into()),
                item_name: Some("B".into()),
                sub_item_name: Some("C".into()),
                period_from: Some("D".into()),
                period_to: Some("E".into()),
                base_amount: Some("F".into()),
                rate: Some("G".into()),
                tax_amount: Some("H".into()),
                ..Default::default()
            },
            fixed_cells: vec![],
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct StampConfirmedQuery {
    pub filing_from: Option<String>,
    pub filing_to: Option<String>,
    pub category_code: Option<String>,
    pub keyword: Option<String>,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ConfirmedStampRow {
    pub run_id: i64,
    pub filing_period: PeriodRange,
    pub voucher_periods: Vec<PeriodRange>,
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
    pub goods_name: String,
    pub category_code: String,
    pub item_name: String,
    pub sub_item_name: Option<String>,
    pub rate_ppm: i64,
    pub base_amount: i64,
    pub stamp_tax: i64,
    pub match_type: MatchType,
    pub hit_rules: Option<String>,
    pub remark: Option<String>,
    pub voucher_nos: Option<String>,
    pub po_nos: Option<String>,
    pub cost_centers: Option<String>,
}

// ─────────────────────────────────────────────────────────────
// 匹配
// ─────────────────────────────────────────────────────────────

const HDR_CTE: &str = "WITH hdr AS (
    SELECT DISTINCT v.header_id FROM v_invoice_vouchers v
    JOIN tax_run_sources rs ON rs.run_id = ?1
      AND v.posted_period BETWEEN rs.src_period_from AND rs.src_period_to
)";

const MATCH_HDR_CTE: &str = "WITH hdr AS (SELECT DISTINCT v.header_id FROM v_invoice_vouchers v WHERE v.posted_period BETWEEN ?2 AND ?3)";

const MANUAL_OVR: &str = "(included IS NOT NULL OR stamp_tax_category_code IS NOT NULL)";

pub fn run_match(conn: &mut Connection, filing: &PeriodRange, source: &PeriodRange, user: &str) -> Result<MatchSummary> {
    let tx = conn.transaction()?;
    let run = get_or_create_run(&tx, M, filing, source, user)?;
    run.ensure_editable()?;
    let rules = rule_engine::load_rules(&tx, SCOPE)?;

    tx.execute(
        &format!(
            "DELETE FROM stamp_tax_results
             WHERE run_id = ?1
               AND detail_id IN (SELECT d.id FROM invoice_details d WHERE d.header_id IN (
                   SELECT v.header_id FROM v_invoice_vouchers v WHERE v.posted_period BETWEEN ?2 AND ?3))
               AND detail_id NOT IN (SELECT detail_id FROM stamp_tax_confirmed_lines)
               AND detail_id NOT IN (SELECT detail_id FROM stamp_tax_overrides WHERE run_id = ?1 AND {MANUAL_OVR})"
        ),
        params![run.id, source.from, source.to],
    )?;

    struct Cand {
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
    let cands: Vec<Cand> = {
        let mut stmt = tx.prepare(&format!(
            "{MATCH_HDR_CTE}
             SELECT d.id, d.header_id, d.tax_code, d.goods_category, d.goods_name, s.tax_no,
                    EXISTS (SELECT 1 FROM stamp_tax_confirmed_lines x WHERE x.detail_id = d.id),
                    EXISTS (SELECT 1 FROM stamp_tax_overrides x WHERE x.detail_id = d.id
                            AND x.run_id = ?1 AND (x.included IS NOT NULL OR x.stamp_tax_category_code IS NOT NULL)),
                    EXISTS (SELECT 1 FROM stamp_tax_results x WHERE x.detail_id = d.id)
                    OR EXISTS (SELECT 1 FROM stamp_tax_overrides x WHERE x.detail_id = d.id AND x.run_id <> ?1 AND x.included = 1)
             FROM hdr
             JOIN invoice_headers h ON h.id = hdr.header_id
             JOIN sellers s         ON s.id = h.seller_id
             JOIN invoice_details d ON d.header_id = h.id"
        ))?;
        let rows = stmt
            .query_map(params![run.id, source.from, source.to], |r| {
                Ok(Cand {
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

    let ts = now();
    let (mut hit_count, mut sk_manual, mut sk_conf, mut sk_other) = (0i64, 0i64, 0i64, 0i64);
    {
        let mut ins_res = tx.prepare_cached(
            "INSERT INTO stamp_tax_results (run_id, detail_id, header_id, matched_at) VALUES (?1, ?2, ?3, ?4)",
        )?;
        let mut ins_hit = tx.prepare_cached("INSERT INTO stamp_tax_rule_hits (result_id, rule_id) VALUES (?1, ?2)")?;
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
// 列表
// ─────────────────────────────────────────────────────────────

fn split_csv(s: Option<String>) -> Vec<String> {
    s.map(|x| x.split(',').map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect())
        .unwrap_or_default()
}

pub fn list_lines(conn: &Connection, run_id: i64) -> Result<StampLinesPayload> {
    let run = load_run(conn, run_id)?;
    run.ensure_module(M)?;
    let cats = category_map(conn)?;

    let sql = format!(
        "{HDR_CTE}
         SELECT r.id AS result_id, r.run_id AS result_run_id, r.matched_at, d.id AS detail_id, d.header_id,
                COALESCE(trim(h.code), '') AS invoice_code, {INVOICE_NO_SQL} AS invoice_no,
                h.issue_date, s.name AS seller_name, s.tax_no AS seller_tax_no,
                d.line_no, d.tax_code, d.goods_category, d.goods_name, d.spec_model,
                d.amount, d.tax_rate, d.tax,
                (SELECT group_concat(DISTINCT v.source || ' ' || v.voucher_no) FROM v_invoice_vouchers v
                  WHERE v.header_id = h.id AND EXISTS (SELECT 1 FROM tax_run_sources rs WHERE rs.run_id = ?1 AND v.posted_period BETWEEN rs.src_period_from AND rs.src_period_to)) AS voucher_nos,
                (SELECT group_concat(DISTINCT v.posted_date) FROM v_invoice_vouchers v
                  WHERE v.header_id = h.id AND EXISTS (SELECT 1 FROM tax_run_sources rs WHERE rs.run_id = ?1 AND v.posted_period BETWEEN rs.src_period_from AND rs.src_period_to)) AS voucher_dates,
                (SELECT group_concat(DISTINCT v.po_no) FROM v_invoice_vouchers v
                  WHERE v.header_id = h.id AND EXISTS (SELECT 1 FROM tax_run_sources rs WHERE rs.run_id = ?1 AND v.posted_period BETWEEN rs.src_period_from AND rs.src_period_to)) AS po_nos,
                (SELECT group_concat(DISTINCT v.cost_center) FROM v_invoice_vouchers v
                  WHERE v.header_id = h.id AND EXISTS (SELECT 1 FROM tax_run_sources rs WHERE rs.run_id = ?1 AND v.posted_period BETWEEN rs.src_period_from AND rs.src_period_to)) AS cost_centers,
                (SELECT group_concat(rh.rule_id) FROM stamp_tax_rule_hits rh WHERE rh.result_id = r.id) AS hit_rule_ids,
                (SELECT group_concat(t.rule_category || '→' || t.stamp_tax_category_code
                         || CASE WHEN t.note <> '' THEN '（' || t.note || '）' ELSE '' END, '；')
                   FROM stamp_tax_rule_hits rh JOIN tax_match_rules t ON t.id = rh.rule_id
                  WHERE rh.result_id = r.id) AS hit_rules,
                (SELECT group_concat(DISTINCT t.stamp_tax_category_code)
                   FROM stamp_tax_rule_hits rh JOIN tax_match_rules t ON t.id = rh.rule_id
                  WHERE rh.result_id = r.id) AS hit_categories,
                (SELECT MAX(t.needs_manual_review)
                   FROM stamp_tax_rule_hits rh JOIN tax_match_rules t ON t.id = rh.rule_id
                  WHERE rh.result_id = r.id) AS hit_needs_review,
                o.included AS ov_included, o.stamp_tax_category_code AS ov_category, o.remark AS ov_remark,
                o.updated_by AS ov_by, o.updated_at AS ov_at,
                cl.category_code AS cl_category, cl.base_amount AS cl_base, cl.rate_ppm AS cl_rate,
                cl.stamp_tax AS cl_tax, cl.decision AS cl_decision, b.batch_no AS cl_batch_no
         FROM hdr
         JOIN invoice_headers h ON h.id = hdr.header_id
         JOIN sellers s         ON s.id = h.seller_id
         JOIN invoice_details d ON d.header_id = h.id
         LEFT JOIN stamp_tax_results r          ON r.detail_id  = d.id
         LEFT JOIN stamp_tax_overrides o        ON o.detail_id  = d.id AND o.run_id = ?1
         LEFT JOIN stamp_tax_confirmed_lines cl ON cl.detail_id = d.id
         LEFT JOIN tax_run_batches b            ON b.id = cl.batch_id
         ORDER BY h.issue_date, h.id, d.line_no"
    );
    let mut stmt = conn.prepare(&sql)?;
    let lines = stmt
        .query_map(params![run.id], |r| {
            let result_id: Option<i64> = r.get("result_id")?;
            let amount: i64 = r.get("amount")?;
            let ov_included: Option<i64> = r.get("ov_included")?;
            let ov_category: Option<String> = r.get("ov_category")?;
            let cl_category: Option<String> = r.get("cl_category")?;
            let hit_categories = split_csv(r.get("hit_categories")?);
            let hit_review: Option<i64> = r.get("hit_needs_review")?;
            let result_run: Option<i64> = r.get("result_run_id")?;
            let foreign_hit = result_id.is_some() && result_run != Some(run.id);
            let auto_hit = result_id.is_some() && !foreign_hit;
            let confirmed = cl_category.is_some();
            let auto_category = if hit_categories.len() == 1 { hit_categories.first().cloned() } else { None };
            let conflict = hit_categories.len() > 1;

            let (final_included, final_category, base, rate, tax, mt) = if let Some(code) = cl_category {
                let rate: i64 = r.get("cl_rate")?;
                (
                    true,
                    Some(code),
                    r.get::<_, i64>("cl_base")?,
                    Some(rate),
                    r.get::<_, i64>("cl_tax")?,
                    MatchType::from_decision(r.get::<_, Option<String>>("cl_decision")?.as_deref().unwrap_or("auto")),
                )
            } else {
                let included = ov_included.map(|v| v == 1).unwrap_or(auto_hit);
                let category = if included { ov_category.clone().or_else(|| auto_category.clone()) } else { None };
                let rate = category.as_ref().and_then(|c| cats.get(c)).and_then(|c| c.rate_ppm);
                let tax = match (included, rate) {
                    (true, Some(rt)) => calc_tax(amount, rt),
                    _ => 0,
                };
                let manual = ov_included.map_or(false, |v| (v == 1) != auto_hit)
                    || (included && ov_category.is_some() && ov_category != auto_category);
                let mt = if manual {
                    MatchType::Manual
                } else if ov_included.is_some() || ov_category.is_some() {
                    MatchType::Reviewed
                } else if auto_hit {
                    MatchType::AutoHit
                } else {
                    MatchType::NoHit
                };
                (included, category, amount, rate, tax, mt)
            };
            let decided = ov_included.is_some() || ov_category.is_some();
            let needs_review = !confirmed && auto_hit && !decided && (hit_review == Some(1) || conflict);
            let category_invalid = !confirmed && final_included && rate.is_none();
            let (item_name, sub_item_name) = match &final_category {
                Some(c) => {
                    let (_, iname, _, sname) = item_and_sub(&cats, c);
                    (Some(iname), sname)
                }
                None => (None, None),
            };

            Ok(StampLine {
                result_id,
                matched_at: r.get("matched_at")?,
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
                spec_model: r.get("spec_model")?,
                amount,
                tax_rate: r.get("tax_rate")?,
                vat_tax: r.get("tax")?,
                voucher_nos: r.get("voucher_nos")?,
                voucher_dates: r.get("voucher_dates")?,
                po_nos: r.get("po_nos")?,
                cost_centers: r.get("cost_centers")?,
                auto_hit,
                matched_in_other_period: foreign_hit,
                hit_rule_ids: split_csv(r.get("hit_rule_ids")?).iter().filter_map(|x| x.parse().ok()).collect(),
                hit_rules: r.get("hit_rules")?,
                hit_categories,
                auto_category,
                override_included: ov_included.map(|v| v == 1),
                override_category: ov_category,
                remark: r.get("ov_remark")?,
                updated_by: r.get("ov_by")?,
                updated_at: r.get("ov_at")?,
                final_included,
                final_category,
                item_name,
                sub_item_name,
                rate_ppm: rate,
                base_amount: if final_included { base } else { 0 },
                stamp_tax: tax,
                match_type: mt,
                needs_review,
                category_invalid,
                confirmed_batch_no: r.get("cl_batch_no")?,
                locked: confirmed,
                pending: final_included && !confirmed,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let stats = compute_stats(conn, &run, &lines)?;
    Ok(StampLinesPayload { run, lines, stats })
}

fn snapshot(pending: &[&StampLine]) -> PendingSnapshot {
    PendingSnapshot {
        fingerprint: fingerprint(pending.iter().map(|l| {
            format!(
                "{}:{}:{}:{}",
                l.detail_id,
                l.final_category.as_deref().unwrap_or(""),
                l.base_amount,
                l.stamp_tax
            )
        })),
        line_count: pending.len() as i64,
        tax_total: pending.iter().map(|l| l.stamp_tax).sum(),
    }
}

fn compute_stats(conn: &Connection, run: &TaxRun, lines: &[StampLine]) -> Result<LineStats> {
    let mut s = LineStats { total_lines: lines.len() as i64, ..Default::default() };
    for l in lines {
        if l.auto_hit {
            s.auto_hit_count += 1;
        }
        if l.final_included {
            s.included_count += 1;
            s.included_tax += l.stamp_tax;
        }
        if l.pending {
            s.pending_count += 1;
            s.pending_tax += l.stamp_tax;
        }
        if l.needs_review {
            s.needs_review_count += 1;
        }
        if l.category_invalid {
            s.invalid_count += 1;
        }
        if l.match_type == MatchType::Manual {
            s.manual_count += 1;
        }
        if l.locked {
            s.confirmed_count += 1;
            s.confirmed_tax += l.stamp_tax;
        }
    }
    if let Some(b) = run.pending_batch() {
        if let Some(fp) = batch_export_fingerprint(conn, b.id)? {
            s.exported = true;
            let pending: Vec<&StampLine> = lines.iter().filter(|l| l.pending).collect();
            s.export_stale = fp != snapshot(&pending).fingerprint;
        }
    }
    Ok(s)
}

fn ensure_ready(stats: &LineStats) -> Result<()> {
    if stats.needs_review_count > 0 {
        bail!("还有 {} 条明细待复核（需人工核对或命中多个印花税类别）", stats.needs_review_count);
    }
    if stats.invalid_count > 0 {
        bail!("还有 {} 条计入明细未指定有效的印花税子目", stats.invalid_count);
    }
    Ok(())
}

// ─────────────────────────────────────────────────────────────
// 人工修改
// ─────────────────────────────────────────────────────────────

pub fn apply_override(
    conn: &mut Connection,
    run_id: i64,
    detail_ids: &[i64],
    action: &StampAction,
    remark: Option<String>,
    user: &str,
) -> Result<ApplyResult> {
    let tx = conn.transaction()?;
    let run = load_run(&tx, run_id)?;
    run.ensure_module(M)?;
    run.ensure_editable()?;
    let cats = category_map(&tx)?;
    if let StampAction::SetCategory { category_code } = action {
        let c = cats.get(category_code).ok_or_else(|| anyhow!("印花税类别 {category_code} 不存在"))?;
        if c.rate_ppm.is_none() {
            bail!("“{}”需要选择具体子目", c.name);
        }
    }
    let remark = remark.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    let ts = now();
    let (mut applied, mut skipped) = (0i64, 0i64);
    {
        let mut info = tx.prepare_cached(&format!(
            "{HDR_CTE}
             SELECT EXISTS (SELECT 1 FROM stamp_tax_results x WHERE x.detail_id = d.id AND x.run_id = ?1),
                    EXISTS (SELECT 1 FROM stamp_tax_confirmed_lines x WHERE x.detail_id = d.id),
                    EXISTS (SELECT 1 FROM stamp_tax_results x WHERE x.detail_id = d.id AND x.run_id <> ?1)
             FROM invoice_details d
             WHERE d.id = ?2 AND d.header_id IN (SELECT header_id FROM hdr)"
        ))?;
        let mut existing = tx.prepare_cached(
            "SELECT included, stamp_tax_category_code, remark FROM stamp_tax_overrides WHERE detail_id = ?1 AND run_id = ?2",
        )?;
        let mut upsert = tx.prepare_cached(
            "INSERT INTO stamp_tax_overrides
                 (run_id, detail_id, included, stamp_tax_category_code, remark, updated_by, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(run_id, detail_id) DO UPDATE SET
                  included = excluded.included, stamp_tax_category_code = excluded.stamp_tax_category_code,
                 remark = excluded.remark, updated_by = excluded.updated_by, updated_at = excluded.updated_at",
        )?;
        let mut del = tx.prepare_cached("DELETE FROM stamp_tax_overrides WHERE detail_id = ?1 AND run_id = ?2")?;

        for &id in detail_ids {
            let row: Option<(bool, bool, bool)> = info
                .query_row(params![run.id, id], |r| {
                    Ok((r.get(0)?, r.get(1)?, r.get(2)?))
                })
                .optional()?;
            let (auto_hit, confirmed, other) =
                row.ok_or_else(|| anyhow!("明细 {id} 不属于当前纳税所属期已选凭证期间"))?;
            if confirmed {
                skipped += 1;
                continue;
            }
            let (mut inc, mut cat, mut rem): (Option<i64>, Option<String>, Option<String>) = existing
                .query_row(params![id, run.id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
                .optional()?
                .unwrap_or((None, None, None));
            match action {
                StampAction::Restore => {
                    del.execute(params![id, run.id])?;
                    applied += 1;
                    continue;
                }
                StampAction::Include => inc = Some(1),
                StampAction::Exclude => {
                    inc = Some(0);
                    cat = None;
                }
                StampAction::SetCategory { category_code } => {
                    inc = Some(1);
                    cat = Some(category_code.clone());
                }
                StampAction::AcceptSystem => {
                    inc = Some(auto_hit as i64);
                }
                StampAction::SetRemark => rem = remark.clone(),
            }
            if !matches!(action, StampAction::SetRemark) && remark.is_some() {
                rem = remark.clone();
            }
            if inc.is_none() && cat.is_none() && rem.is_none() {
                del.execute([id])?;
            } else {
                upsert.execute(params![run.id, id, inc, cat, rem, user, ts])?;
            }
            applied += 1;
        }
    }
    touch_run(&tx, run.id, false)?;
    tx.commit()?;
    Ok(ApplyResult { applied, skipped_locked: skipped })
}

// ─────────────────────────────────────────────────────────────
// 导出清单
// ─────────────────────────────────────────────────────────────

const LINE_COLS: &[Col] = &[
    col("结果ID", 8.0),
    col("明细ID", 8.0),
    col("发票头ID", 9.0),
    col("匹配时间", 18.0),
    col("发票代码", 13.0),
    col("发票号码", 22.0),
    col("开票日期", 11.0),
    col("销售方名称", 30.0),
    col("销售方税号", 21.0),
    col("行号", 6.0),
    col("税收分类编码", 21.0),
    col("货物或应税劳务名称", 28.0),
    col("计税金额(不含税)", 14.0),
    col("增值税税额", 12.0),
    col("印花税税目", 16.0),
    col("子目", 16.0),
    col("类别代码", 10.0),
    col("税率", 10.0),
    col("印花税额", 12.0),
    col("匹配类型", 14.0),
    col("命中规则", 30.0),
    col("凭证(来源 凭证号)", 26.0),
    col("入账日期", 22.0),
    col("PO", 16.0),
    col("成本中心", 14.0),
    col("备注", 24.0),
    col("确认批次", 8.0),
];

fn line_row(l: &StampLine) -> Vec<Val> {
    vec![
        l.result_id.map(Val::Int).unwrap_or(Val::Empty),
        Val::Int(l.detail_id),
        Val::Int(l.header_id),
        l.matched_at.clone().into(),
        l.invoice_code.clone().into(),
        l.invoice_no.clone().into(),
        l.issue_date.clone().into(),
        l.seller_name.clone().into(),
        l.seller_tax_no.clone().into(),
        Val::Int(l.line_no),
        l.tax_code.clone().into(),
        l.goods_name.clone().into(),
        Val::Money(l.base_amount),
        Val::Money(l.vat_tax),
        l.item_name.clone().into(),
        l.sub_item_name.clone().into(),
        l.final_category.clone().into(),
        l.rate_ppm.map(|r| Val::Num(r as f64 / 1_000_000.0)).unwrap_or(Val::Empty),
        Val::Money(l.stamp_tax),
        l.match_type.label().into(),
        l.hit_rules.clone().into(),
        l.voucher_nos.clone().into(),
        l.voucher_dates.clone().into(),
        l.po_nos.clone().into(),
        l.cost_centers.clone().into(),
        l.remark.clone().into(),
        l.confirmed_batch_no.map(Val::Int).unwrap_or(Val::Empty),
    ]
}

/// 由明细行做子目汇总
fn summarize_lines<'a>(
    cats: &HashMap<String, StampCategory>,
    lines: impl Iterator<Item = (&'a str, i64, i64)>, // (category_code, rate_ppm, base)
    level: GroupLevel,
) -> Vec<StampSummaryRow> {
    let mut child: BTreeMap<(String, i64), (i64, i64)> = BTreeMap::new();
    for (code, rate, base) in lines {
        let e = child.entry((code.to_string(), rate)).or_insert((0, 0));
        e.0 += 1;
        e.1 += base;
    }
    let child_rows: Vec<StampSummaryRow> = child
        .into_iter()
        .map(|((code, rate), (n, base))| {
            let (ic, iname, sc, sname) = item_and_sub(cats, &code);
            StampSummaryRow {
                item_code: ic,
                item_name: iname,
                sub_item_code: sc,
                sub_item_name: sname,
                rate_ppm: Some(rate),
                line_count: n,
                base_amount: base,
                tax_amount: calc_tax(base, rate),
            }
        })
        .collect();
    if level == GroupLevel::Child {
        return child_rows;
    }
    let mut parent: BTreeMap<String, StampSummaryRow> = BTreeMap::new();
    for r in child_rows {
        let e = parent.entry(r.item_code.clone()).or_insert_with(|| StampSummaryRow {
            item_code: r.item_code.clone(),
            item_name: r.item_name.clone(),
            sub_item_code: None,
            sub_item_name: None,
            rate_ppm: r.rate_ppm,
            line_count: 0,
            base_amount: 0,
            tax_amount: 0,
        });
        if e.rate_ppm != r.rate_ppm {
            e.rate_ppm = None; // 子目税率不同，父级不显示单一税率
        }
        e.line_count += r.line_count;
        e.base_amount += r.base_amount;
        e.tax_amount += r.tax_amount;
    }
    parent.into_values().collect()
}

const SUM_COLS: [Col; 7] = [
    col("税目", 18.0),
    col("子目", 18.0),
    col("税率", 10.0),
    col("行数", 8.0),
    col("计税金额", 16.0),
    col("印花税额", 14.0),
    col("类别代码", 12.0),
];

fn summary_val_rows(rows: &[StampSummaryRow]) -> Vec<Vec<Val>> {
    rows.iter()
        .map(|r| {
            vec![
                Val::Text(r.item_name.clone()),
                r.sub_item_name.clone().into(),
                r.rate_ppm.map(|x| Val::Num(x as f64 / 1_000_000.0)).unwrap_or(Val::Empty),
                Val::Int(r.line_count),
                Val::Money(r.base_amount),
                Val::Money(r.tax_amount),
                Val::Text(r.sub_item_code.clone().unwrap_or_else(|| r.item_code.clone())),
            ]
        })
        .collect()
}

fn write_xlsx(
    conn: &Connection,
    path: &str,
    run: &TaxRun,
    batch: &RunBatch,
    lines: &[&StampLine],
    user: &str,
) -> Result<()> {
    let cats = category_map(conn)?;
    let mut wb = Workbook::new();
    let rows: Vec<Vec<Val>> = lines.iter().map(|l| line_row(l)).collect();
    write_table(&mut wb, "印花税明细", LINE_COLS, &rows)?;
    let summary = summarize_lines(
        &cats,
        lines.iter().filter_map(|l| Some((l.final_category.as_deref()?, l.rate_ppm?, l.base_amount))),
        GroupLevel::Child,
    );
    let info = vec![
        ("模块", M.label().to_string()),
        ("凭证期间范围", run.src_periods.iter().map(PeriodRange::label).collect::<Vec<_>>().join("；")),
        ("纳税所属期", run.filing_period.as_ref().map(|p| p.label()).unwrap_or_default()),
        ("批次", format!("{}（{}）", batch.batch_no, if batch.batch_no == 1 { "首次" } else { "补充/补税" })),
        ("明细行数", lines.len().to_string()),
        ("计税金额合计(元)", format!("{:.2}", fen_to_yuan(lines.iter().map(|l| l.base_amount).sum()))),
        ("印花税合计(元，按子目汇总计算)", format!("{:.2}", fen_to_yuan(summary.iter().map(|r| r.tax_amount).sum()))),
        ("会计凭证号", batch.acct_voucher_no.clone().unwrap_or_default()),
        ("入账日期", batch.acct_posted_date.clone().unwrap_or_default()),
        ("做账人", batch.acct_preparer.clone().unwrap_or_default()),
        ("导出人", user.to_string()),
        ("导出时间", now()),
    ];
    write_info_sheet(&mut wb, "说明及汇总", &info, &SUM_COLS, &summary_val_rows(&summary))?;
    wb.save(path)?;
    Ok(())
}

pub fn export_pending_batch(conn: &Connection, run_id: i64, path: &str, user: &str) -> Result<ExportResult> {
    let p = list_lines(conn, run_id)?;
    p.run.ensure_editable()?;
    ensure_ready(&p.stats)?;
    let batch = p.run.pending_batch().cloned().ok_or_else(|| anyhow!("没有待确认的批次"))?;
    let pending: Vec<&StampLine> = p.lines.iter().filter(|l| l.pending).collect();
    if batch.batch_no > 1 && pending.is_empty() {
        bail!("补充批次没有新增印花税明细");
    }
    write_xlsx(conn, path, &p.run, &batch, &pending, user)?;
    let snap = snapshot(&pending);
    record_export(conn, &p.run, &snap, path, user)?;
    Ok(ExportResult { path: path.into(), line_count: snap.line_count, tax_total: snap.tax_total })
}

pub fn export_confirmed_batch(conn: &Connection, run_id: i64, batch_no: i64, path: &str, user: &str) -> Result<ExportResult> {
    let p = list_lines(conn, run_id)?;
    let batch = p
        .run
        .batches
        .iter()
        .find(|b| b.batch_no == batch_no && b.confirmed_at.is_some())
        .cloned()
        .ok_or_else(|| anyhow!("批次 {batch_no} 不存在或尚未确认"))?;
    let lines: Vec<&StampLine> = p.lines.iter().filter(|l| l.confirmed_batch_no == Some(batch_no)).collect();
    write_xlsx(conn, path, &p.run, &batch, &lines, user)?;
    Ok(ExportResult {
        path: path.into(),
        line_count: lines.len() as i64,
        tax_total: lines.iter().map(|l| l.stamp_tax).sum(),
    })
}

// ─────────────────────────────────────────────────────────────
// 确认
// ─────────────────────────────────────────────────────────────

pub fn confirm(conn: &mut Connection, input: &ConfirmInput, user: &str) -> Result<TaxRun> {
    let tx = conn.transaction()?;
    let p = list_lines(&tx, input.run_id)?;
    ensure_ready(&p.stats)?;
    let pending: Vec<&StampLine> = p.lines.iter().filter(|l| l.pending).collect();
    let snap = snapshot(&pending);
    let (run, batch, voucher) = begin_confirm(&tx, M, input, &snap)?;
    let ts = now();
    {
        let mut ins = tx.prepare_cached(
            "INSERT INTO stamp_tax_confirmed_lines
                 (run_id, batch_id, detail_id, header_id, category_code, base_amount, rate_ppm, stamp_tax,
                  decision, remark, confirmed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        )?;
        for l in &pending {
            let code = l.final_category.as_ref().ok_or_else(|| anyhow!("明细 {} 缺少类别", l.detail_id))?;
            let rate = l.rate_ppm.ok_or_else(|| anyhow!("明细 {} 缺少税率", l.detail_id))?;
            ins.execute(params![
                run.id,
                batch.id,
                l.detail_id,
                l.header_id,
                code,
                l.base_amount,
                rate,
                l.stamp_tax,
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

pub fn pending_count(conn: &Connection, run_id: i64) -> Result<i64> {
    Ok(list_lines(conn, run_id)?.stats.pending_count)
}

pub fn delete_run_data(conn: &Connection, run_id: i64) -> Result<()> {
    conn.execute("DELETE FROM stamp_tax_overrides WHERE run_id = ?1", [run_id])?;
    conn.execute("DELETE FROM stamp_tax_results WHERE run_id = ?1", [run_id])?;
    Ok(())
}

// ─────────────────────────────────────────────────────────────
// 汇总 & 印花税信息采集表
// ─────────────────────────────────────────────────────────────

/// 某纳税所属期（所属期批次）的汇总。batch_no = None 表示全部已确认批次；
/// include_pending = true 时把尚未确认的计入行一起汇总（确认前预览用）
pub fn summary(
    conn: &Connection,
    run_id: i64,
    level: GroupLevel,
    batch_no: Option<i64>,
    include_pending: bool,
) -> Result<Vec<StampSummaryRow>> {
    let cats = category_map(conn)?;
    let p = list_lines(conn, run_id)?;
    let it = p.lines.iter().filter(|l| {
        if l.locked {
            batch_no.map_or(true, |b| l.confirmed_batch_no == Some(b))
        } else {
            include_pending && l.pending
        }
    });
    Ok(summarize_lines(
        &cats,
        it.filter_map(|l| Some((l.final_category.as_deref()?, l.rate_ppm?, l.base_amount))),
        level,
    ))
}

pub fn get_template_settings(conn: &Connection) -> Result<StampTemplateSettings> {
    let v: Option<String> = conn
        .query_row("SELECT value FROM app_settings WHERE key = ?1", [TEMPLATE_KEY], |r| r.get(0))
        .optional()?;
    match v {
        Some(s) => serde_json::from_str(&s).context("印花税采集表模版设置损坏"),
        None => Ok(StampTemplateSettings::default()),
    }
}

pub fn save_template_settings(conn: &Connection, s: &StampTemplateSettings, user: &str) -> Result<()> {
    if s.start_row < 1 {
        bail!("数据起始行必须 ≥ 1");
    }
    let c = &s.columns;
    for (label, v) in [
        ("序号", &c.seq),
        ("税目代码", &c.item_code),
        ("税目", &c.item_name),
        ("子目代码", &c.sub_item_code),
        ("子目", &c.sub_item_name),
        ("所属期起", &c.period_from),
        ("所属期止", &c.period_to),
        ("计税金额", &c.base_amount),
        ("税率", &c.rate),
        ("应纳税额", &c.tax_amount),
        ("行数", &c.line_count),
    ] {
        if let Some(col) = v.as_deref().map(str::trim).filter(|x| !x.is_empty()) {
            if xlsx_template::parse_col_letters(col).is_none() {
                bail!("“{label}”列号无效：{col}");
            }
        }
    }
    for f in &s.fixed_cells {
        if xlsx_template::parse_cell_ref(&f.cell).is_none() {
            bail!("固定单元格地址无效：{}", f.cell);
        }
    }
    conn.execute(
        "INSERT INTO app_settings (key, value, updated_by, updated_at) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_by = excluded.updated_by,
                                        updated_at = excluded.updated_at",
        params![TEMPLATE_KEY, serde_json::to_string(s)?, user, now()],
    )?;
    Ok(())
}

/// 按税务局模版导出某纳税所属期的印花税信息采集表
pub fn export_collection_form(
    conn: &Connection,
    run_id: i64,
    batch_no: Option<i64>,
    output: &str,
    user: &str,
) -> Result<ExportResult> {
    let settings = get_template_settings(conn)?;
    let template = settings
        .template_path
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| anyhow!("请先在“采集表模版设置”中选择税务局模版文件"))?;
    let run = load_run(conn, run_id)?;
    run.ensure_module(M)?;
    let filing = run
        .filing_period
        .clone()
        .ok_or_else(|| anyhow!("该所属期批次尚未确认纳税所属期，不能导出采集表"))?;
    let rows = summary(conn, run_id, settings.group_level, batch_no, false)?;
    if rows.is_empty() {
        bail!("该纳税所属期没有已确认的印花税明细");
    }

    let mut cells = CellMap::new();
    let c = &settings.columns;
    let start = settings.start_row as u32;
    let put = |cells: &mut CellMap, col: &Option<String>, row: u32, v: CellValue| -> Result<()> {
        if let Some(letters) = col.as_deref().map(str::trim).filter(|x| !x.is_empty()) {
            set_cell(cells, &format!("{letters}{row}"), v)?;
        }
        Ok(())
    };
    for (i, r) in rows.iter().enumerate() {
        let row = start + i as u32;
        put(&mut cells, &c.seq, row, CellValue::Number((i + 1) as f64))?;
        put(&mut cells, &c.item_code, row, CellValue::Text(r.item_code.clone()))?;
        put(&mut cells, &c.item_name, row, CellValue::Text(r.item_name.clone()))?;
        put(&mut cells, &c.sub_item_code, row, CellValue::Text(r.sub_item_code.clone().unwrap_or_default()))?;
        put(&mut cells, &c.sub_item_name, row, CellValue::Text(r.sub_item_name.clone().unwrap_or_default()))?;
        put(&mut cells, &c.period_from, row, CellValue::Text(filing.first_day()))?;
        put(&mut cells, &c.period_to, row, CellValue::Text(filing.last_day()))?;
        put(&mut cells, &c.base_amount, row, CellValue::Number(fen_to_yuan(r.base_amount)))?;
        if let Some(rate) = r.rate_ppm {
            put(&mut cells, &c.rate, row, CellValue::Number(rate as f64 / 1_000_000.0))?;
        }
        put(&mut cells, &c.tax_amount, row, CellValue::Number(fen_to_yuan(r.tax_amount)))?;
        put(&mut cells, &c.line_count, row, CellValue::Number(r.line_count as f64))?;
    }

    let total_base: i64 = rows.iter().map(|r| r.base_amount).sum();
    let total_tax: i64 = rows.iter().map(|r| r.tax_amount).sum();
    for f in &settings.fixed_cells {
        let v = f
            .value
            .replace("{filingFrom}", &filing.from)
            .replace("{filingTo}", &filing.to)
            .replace("{periodStart}", &filing.first_day())
            .replace("{periodEnd}", &filing.last_day())
            .replace("{totalBase}", &format!("{:.2}", fen_to_yuan(total_base)))
            .replace("{totalTax}", &format!("{:.2}", fen_to_yuan(total_tax)))
            .replace("{today}", &chrono::Local::now().format("%Y-%m-%d").to_string())
            .replace("{user}", user);
        set_cell(&mut cells, &f.cell, CellValue::Text(v))?;
    }

    xlsx_template::fill_template(Path::new(template), Path::new(output), settings.sheet_name.as_deref(), &cells)?;
    Ok(ExportResult { path: output.into(), line_count: rows.len() as i64, tax_total: total_tax })
}

// ─────────────────────────────────────────────────────────────
// 已确认查询
// ─────────────────────────────────────────────────────────────

pub fn query_confirmed(conn: &Connection, q: &StampConfirmedQuery) -> Result<Vec<ConfirmedStampRow>> {
    let cats = category_map(conn)?;
    let kw = q
        .keyword
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| format!("%{}%", s.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_")));
    let cat = q.category_code.as_ref().map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    let sql = format!(
        "SELECT tr.id AS run_id, tr.filing_period_from, tr.filing_period_to,
                (SELECT group_concat(rs.src_period_from || ':' || rs.src_period_to, ';') FROM tax_run_sources rs WHERE rs.run_id = tr.id) AS source_ranges,
                b.batch_no, b.acct_voucher_no, b.acct_posted_date, b.acct_preparer, b.confirmed_by, b.confirmed_at,
                cl.detail_id, cl.header_id, COALESCE(trim(h.code), '') AS invoice_code, {INVOICE_NO_SQL} AS invoice_no,
                h.issue_date, s.name AS seller_name, s.tax_no AS seller_tax_no, d.line_no, d.goods_name,
                cl.category_code, cl.rate_ppm, cl.base_amount, cl.stamp_tax, cl.decision, cl.remark,
                (SELECT group_concat(t.rule_category || '→' || t.stamp_tax_category_code, '；')
                   FROM stamp_tax_results r JOIN stamp_tax_rule_hits rh ON rh.result_id = r.id
                   JOIN tax_match_rules t ON t.id = rh.rule_id WHERE r.detail_id = cl.detail_id) AS hit_rules,
                (SELECT group_concat(DISTINCT v.source || ' ' || v.voucher_no) FROM v_invoice_vouchers v
                  WHERE v.header_id = cl.header_id AND EXISTS (SELECT 1 FROM tax_run_sources rs WHERE rs.run_id = tr.id AND v.posted_period BETWEEN rs.src_period_from AND rs.src_period_to)) AS voucher_nos,
                (SELECT group_concat(DISTINCT v.po_no) FROM v_invoice_vouchers v
                  WHERE v.header_id = cl.header_id AND EXISTS (SELECT 1 FROM tax_run_sources rs WHERE rs.run_id = tr.id AND v.posted_period BETWEEN rs.src_period_from AND rs.src_period_to)) AS po_nos,
                (SELECT group_concat(DISTINCT v.cost_center) FROM v_invoice_vouchers v
                  WHERE v.header_id = cl.header_id AND EXISTS (SELECT 1 FROM tax_run_sources rs WHERE rs.run_id = tr.id AND v.posted_period BETWEEN rs.src_period_from AND rs.src_period_to)) AS cost_centers
         FROM stamp_tax_confirmed_lines cl
         JOIN tax_runs tr       ON tr.id = cl.run_id
         JOIN tax_run_batches b ON b.id  = cl.batch_id
         JOIN invoice_details d ON d.id  = cl.detail_id
         JOIN invoice_headers h ON h.id  = cl.header_id
         JOIN sellers s         ON s.id  = h.seller_id
         LEFT JOIN stamp_tax_categories sc ON sc.code = cl.category_code
         WHERE (?1 IS NULL OR tr.filing_period_to   >= ?1)
           AND (?2 IS NULL OR tr.filing_period_from <= ?2)
           AND (?3 IS NULL OR cl.category_code = ?3 OR sc.parent_code = ?3)
           AND (?4 IS NULL OR h.inv_unq_key LIKE ?4 {LIKE_ESCAPE} OR s.name LIKE ?4 {LIKE_ESCAPE}
                OR d.goods_name LIKE ?4 {LIKE_ESCAPE} OR b.acct_voucher_no LIKE ?4 {LIKE_ESCAPE})
         ORDER BY tr.filing_period_from, b.batch_no, cl.category_code, h.issue_date, h.id, d.line_no"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map(params![q.filing_from, q.filing_to, cat, kw], |r| {
            let code: String = r.get("category_code")?;
            let (_, iname, _, sname) = item_and_sub(&cats, &code);
            Ok(ConfirmedStampRow {
                run_id: r.get("run_id")?,
                filing_period: PeriodRange { from: r.get("filing_period_from")?, to: r.get("filing_period_to")? },
                voucher_periods: r.get::<_, Option<String>>("source_ranges")?.unwrap_or_default()
                    .split(';').filter_map(|part| {
                        let (from, to) = part.split_once(':')?;
                        Some(PeriodRange { from: from.to_string(), to: to.to_string() })
                    }).collect(),
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
                goods_name: r.get("goods_name")?,
                category_code: code,
                item_name: iname,
                sub_item_name: sname,
                rate_ppm: r.get("rate_ppm")?,
                base_amount: r.get("base_amount")?,
                stamp_tax: r.get("stamp_tax")?,
                match_type: MatchType::from_decision(&r.get::<_, String>("decision")?),
                hit_rules: r.get("hit_rules")?,
                remark: r.get("remark")?,
                voucher_nos: r.get("voucher_nos")?,
                po_nos: r.get("po_nos")?,
                cost_centers: r.get("cost_centers")?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn export_confirmed_query(conn: &Connection, q: &StampConfirmedQuery, path: &str) -> Result<ExportResult> {
    let rows = query_confirmed(conn, q)?;
    let cols = [
        col("纳税所属期", 14.0),
        col("凭证期间", 14.0),
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
        col("货物或应税劳务名称", 28.0),
        col("税目", 16.0),
        col("子目", 16.0),
        col("税率", 10.0),
        col("计税金额", 14.0),
        col("印花税额", 12.0),
        col("匹配类型", 14.0),
        col("命中规则", 30.0),
        col("原凭证", 26.0),
        col("PO", 16.0),
        col("成本中心", 14.0),
        col("备注", 24.0),
    ];
    let data: Vec<Vec<Val>> = rows
        .iter()
        .map(|r| {
            vec![
                r.filing_period.label().into(),
                r.voucher_periods.iter().map(PeriodRange::label).collect::<Vec<_>>().join("；").into(),
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
                r.goods_name.clone().into(),
                r.item_name.clone().into(),
                r.sub_item_name.clone().into(),
                Val::Num(r.rate_ppm as f64 / 1_000_000.0),
                Val::Money(r.base_amount),
                Val::Money(r.stamp_tax),
                r.match_type.label().into(),
                r.hit_rules.clone().into(),
                r.voucher_nos.clone().into(),
                r.po_nos.clone().into(),
                r.cost_centers.clone().into(),
                r.remark.clone().into(),
            ]
        })
        .collect();
    let mut wb = Workbook::new();
    write_table(&mut wb, "已确认印花税", &cols, &data)?;
    wb.save(path)?;
    Ok(ExportResult {
        path: path.into(),
        line_count: rows.len() as i64,
        tax_total: rows.iter().map(|r| r.stamp_tax).sum(),
    })
}

#[cfg(test)]
mod tests {
    use super::calc_tax;

    #[test]
    fn rounding() {
        // 100000.00 元 × 万分之三 = 30.00 元
        assert_eq!(calc_tax(10_000_000, 300), 3_000);
        // 1234.56 元 × 万分之三 = 0.370368 → 0.37
        assert_eq!(calc_tax(123_456, 300), 37);
        assert_eq!(calc_tax(-123_456, 300), -37);
    }
}
