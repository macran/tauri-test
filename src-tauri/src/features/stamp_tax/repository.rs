use std::collections::{HashMap, HashSet};

use rusqlite::{params, params_from_iter, OptionalExtension, Transaction};

use super::models::{MatchCandidate, RawReviewRow, RuleForMatch};
use crate::features::period_close::repository::{read_confirm, CONFIRM_COLS};

// ───────────────── 做账日期判定 ─────────────────
//
// 印花税按「凭证入账期间」判定，而非认证清单的认证期间——
// 很多发票不走增值税认证，但照样在 AP/Bank/IC/差旅 四张凭证表之一做账，必须纳入印花税考量。
// 一张发票只要在任一凭证表里有凭证号，就算做账；取号时按 AP > Bank > IC > 差旅 优先级，
// 多个来源都命中时取优先级最高且日期最早的一条展示。
//
// 四个来源的日期字段不保证都非空（历史数据/旧版导入），各自退化到一个必然非空的日期列：
//   AP   entry_date  → received_date
//   Bank posted_date（建表时已 NOT NULL）
//   IC   posted_date → received_date
//   差旅 posted_date → check_time（认证清单必填）
// booked_date 取前 10 位（YYYY-MM-DD），与前端传入的日期范围直接做字符串比较。
// 前三个来源的日期都经过 excel_date_to_string 规整为 YYYY-MM-DD；差旅的回退值 check_time
// 没有规整，所以单独把斜杠换成横线。局限：月日不补零的 2026/9/8 仍会比较错位，
// 遇到这种数据源时应在认证清单导入处规整，而不是在这里继续打补丁。
const BOOKED_CTE: &str = "
WITH booked AS (
    SELECT v.header_id AS header_id, v.voucher_no AS voucher_no,
           substr(COALESCE(v.entry_date, v.received_date), 1, 10) AS booked_date,
           1 AS priority
    FROM ap_invoice_vouchers v
    WHERE v.header_id IS NOT NULL
      AND TRIM(COALESCE(v.voucher_no, '')) <> ''
      AND COALESCE(v.entry_date, v.received_date) IS NOT NULL

    UNION ALL

    SELECT v.header_id, v.voucher_no,
           substr(v.posted_date, 1, 10),
           2
    FROM bank_invoice_vouchers v
    WHERE TRIM(COALESCE(v.voucher_no, '')) <> ''
      AND v.posted_date IS NOT NULL

    UNION ALL

    SELECT v.header_id, v.document_no,
           substr(COALESCE(v.posted_date, v.received_date), 1, 10),
           3
    FROM ic_vouchers v
    WHERE TRIM(COALESCE(v.document_no, '')) <> ''
      AND COALESCE(v.posted_date, v.received_date) IS NOT NULL

    UNION ALL

    SELECT cl.header_id, cl.voucher_no,
           -- check_time 是税局导出的原样文本，没有日期规整：斜杠统一成横线再取前 10 位
           substr(replace(COALESCE(cl.posted_date, cl.check_time), '/', '-'), 1, 10),
           4
    FROM certified_list cl
    WHERE COALESCE(cl.is_travel_expense, 0) = 1
      AND TRIM(COALESCE(cl.voucher_no, '')) <> ''
      AND COALESCE(cl.posted_date, cl.check_time) IS NOT NULL
)
";

// ───────────────── 匹配候选 ─────────────────

/// 凭证入账期间 YYYYMM。booked_date 是 YYYY-MM-DD，斜杠日期已在 BOOKED_CTE 里规整成横线。
const BOOKED_PERIOD: &str = "substr(replace(b.booked_date, '-', ''), 1, 6)";

/// 取出凭证入账期间范围内「已做账」的发票明细。
/// 已在任何纳税所属期**确认**（含已锁定）计入的明细不再参与；其余明细——包括别的所属期里
/// 机器命中但没确认的——都可以参与。
pub(crate) fn fetch_match_candidates(
    tx: &Transaction<'_>,
    period_from: &str,
    period_to: &str,
) -> Result<Vec<MatchCandidate>, String> {
    let sql = format!(
        "{BOOKED_CTE}
         SELECT DISTINCT d.id, d.header_id, s.tax_no, d.tax_code, d.goods_category
         FROM invoice_details d
         JOIN invoice_headers h ON h.id = d.header_id
         JOIN sellers s         ON s.id = h.seller_id
         JOIN booked b          ON b.header_id = d.header_id
         WHERE {BOOKED_PERIOD} BETWEEN ?1 AND ?2
           AND NOT EXISTS (SELECT 1 FROM stamp_tax_confirmed_lines c WHERE c.detail_id = d.id)"
    );

    let mut statement = tx
        .prepare(&sql)
        .map_err(|error| format!("准备匹配候选查询失败：{error}"))?;

    let rows = statement
        .query_map(params![period_from, period_to], |row| {
            Ok(MatchCandidate {
                detail_id: row.get(0)?,
                header_id: row.get(1)?,
                seller_tax_no: row.get(2)?,
                tax_code: row.get(3)?,
                goods_category: row.get(4)?,
            })
        })
        .map_err(|error| format!("查询匹配候选失败：{error}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("读取匹配候选失败：{error}"))
}

// ───────────────── 规则 ─────────────────

pub(crate) fn fetch_enabled_rules(tx: &Transaction<'_>) -> Result<Vec<RuleForMatch>, String> {
    fetch_rules(tx, true)
}

/// enabled_only = false 时把禁用规则也读出来，
/// 用于渲染历史命中——规则禁用后，其条件与备注仍要可见。
pub(crate) fn fetch_rules(
    tx: &Transaction<'_>,
    enabled_only: bool,
) -> Result<Vec<RuleForMatch>, String> {
    let sql = if enabled_only {
        "SELECT r.id, r.rule_category, r.stamp_tax_category_code, c.name, c.rate_ppm, r.note, r.needs_manual_review, r.enabled,
                r.seller_tax_id_match_type, r.seller_tax_id_values,
                r.tax_code_match_type, r.tax_code_values,
                r.tax_name_match_type, r.tax_name_values
         FROM tax_match_rules r
         LEFT JOIN stamp_tax_categories c ON c.code = r.stamp_tax_category_code
         WHERE r.scope = 'stamp_tax' AND r.enabled = 1
         ORDER BY r.id"
    } else {
        "SELECT r.id, r.rule_category, r.stamp_tax_category_code, c.name, c.rate_ppm, r.note, r.needs_manual_review, r.enabled,
                r.seller_tax_id_match_type, r.seller_tax_id_values,
                r.tax_code_match_type, r.tax_code_values,
                r.tax_name_match_type, r.tax_name_values
         FROM tax_match_rules r
         LEFT JOIN stamp_tax_categories c ON c.code = r.stamp_tax_category_code
         WHERE r.scope = 'stamp_tax'
         ORDER BY r.id"
    };

    let mut statement = tx
        .prepare(sql)
        .map_err(|error| format!("准备规则查询失败：{error}"))?;

    let rows = statement
        .query_map([], |row| {
            Ok(RuleForMatch {
                id: row.get(0)?,
                rule_category: row.get(1)?,
                stamp_tax_category_code: row.get(2)?,
                stamp_tax_category: row.get(3)?,
                rate_ppm: row.get(4)?,
                note: row.get(5)?,
                needs_manual_review: row.get::<_, i64>(6)? != 0,
                enabled: row.get::<_, i64>(7)? != 0,
                seller_tax_id_match_type: row.get(8)?,
                seller_tax_id_values: row.get(9)?,
                tax_code_match_type: row.get(10)?,
                tax_code_values: row.get(11)?,
                tax_name_match_type: row.get(12)?,
                tax_name_values: row.get(13)?,
            })
        })
        .map_err(|error| format!("查询规则失败：{error}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("读取规则失败：{error}"))
}

// ───────────────── 系统结果写入 ─────────────────

/// 删除本纳税所属期里本次不再命中的系统结果；rule_hits 由 ON DELETE CASCADE 连带清除。
/// 仍然命中的行保留（id 不变），已确认的明细不动，不触碰 stamp_tax_overrides。
pub(crate) fn delete_stale_results(
    tx: &Transaction<'_>,
    run_id: i64,
    keep_detail_ids: &HashSet<i64>,
) -> Result<(), String> {
    let existing: Vec<(i64, i64)> = {
        let mut statement = tx
            .prepare(
                "SELECT r.id, r.detail_id FROM stamp_tax_results r
                 WHERE r.run_id = ?1
                   AND NOT EXISTS (SELECT 1 FROM stamp_tax_confirmed_lines c
                                    WHERE c.detail_id = r.detail_id AND c.run_id = r.run_id)",
            )
            .map_err(|error| format!("准备旧匹配结果查询失败：{error}"))?;
        let rows = statement
            .query_map(params![run_id], |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(|error| format!("查询旧匹配结果失败：{error}"))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("读取旧匹配结果失败：{error}"))?
    };

    for (id, detail_id) in existing {
        if !keep_detail_ids.contains(&detail_id) {
            tx.execute("DELETE FROM stamp_tax_results WHERE id = ?1", params![id])
                .map_err(|error| format!("清除旧匹配结果失败：{error}"))?;
        }
    }
    Ok(())
}

/// 写入或刷新一条系统结果，并重建它的命中规则（规则可能新增）。
pub(crate) fn upsert_result(
    tx: &Transaction<'_>,
    candidate: &MatchCandidate,
    run_id: i64,
    matched_at: &str,
    rule_ids: &[i64],
) -> Result<(), String> {
    // 结果没变（已有这条结果，命中的规则集合也相同）就原样保留，不刷新 matched_at。
    let existing: Option<i64> = tx
        .query_row(
            "SELECT id FROM stamp_tax_results WHERE run_id = ?1 AND detail_id = ?2",
            params![run_id, candidate.detail_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| format!("读取已有匹配结果失败：{error}"))?;
    if let Some(id) = existing {
        let mut old: Vec<i64> = tx
            .prepare("SELECT rule_id FROM stamp_tax_rule_hits WHERE result_id = ?1")
            .and_then(|mut st| st.query_map(params![id], |row| row.get(0))?.collect())
            .map_err(|error| format!("读取已有命中规则失败：{error}"))?;
        let mut new = rule_ids.to_vec();
        old.sort_unstable();
        new.sort_unstable();
        new.dedup();
        if old == new {
            return Ok(());
        }
    }

    tx.execute(
        "INSERT INTO stamp_tax_results (run_id, detail_id, header_id, matched_at)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(run_id, detail_id) DO UPDATE SET matched_at = excluded.matched_at",
        params![run_id, candidate.detail_id, candidate.header_id, matched_at],
    )
    .map_err(|error| format!("保存匹配结果失败：{error}"))?;

    let result_id: i64 = tx
        .query_row(
            "SELECT id FROM stamp_tax_results WHERE run_id = ?1 AND detail_id = ?2",
            params![run_id, candidate.detail_id],
            |row| row.get(0),
        )
        .map_err(|error| format!("读取匹配结果 ID 失败：{error}"))?;

    tx.execute(
        "DELETE FROM stamp_tax_rule_hits WHERE result_id = ?1",
        params![result_id],
    )
    .map_err(|error| format!("重置命中规则失败：{error}"))?;

    let mut statement = tx
        .prepare_cached("INSERT INTO stamp_tax_rule_hits (result_id, rule_id) VALUES (?1, ?2)")
        .map_err(|error| format!("准备命中规则写入失败：{error}"))?;

    for rule_id in rule_ids {
        statement
            .execute(params![result_id, rule_id])
            .map_err(|error| format!("保存命中规则失败：{error}"))?;
    }

    Ok(())
}

/// 统计该纳税所属期已有的人工结论条数，向用户说明有多少条不受重算影响。
pub(crate) fn count_overrides(tx: &Transaction<'_>, run_id: i64) -> Result<usize, String> {
    tx.query_row(
        "SELECT COUNT(*) FROM stamp_tax_overrides WHERE run_id = ?1",
        params![run_id],
        |row| row.get::<_, i64>(0),
    )
    .map(|count| count as usize)
    .map_err(|error| format!("统计人工结论失败：{error}"))
}

// ───────────────── 核对列表主干查询 ─────────────────

/// 行集 = 本纳税所属期来源范围（凭证入账期间）内已做账的明细 ∪ 本所属期已有结论的明细。
/// 后者保证缩小或改了范围后，已入库的行也不会消失。
/// 已确认（本所属期或别的所属期）的明细都显示，别的所属期确认的在前端标记为锁定。
/// 一张发票可能同时被多张凭证表命中（极少见），按 AP > Bank > IC > 差旅 优先级去重，
/// 取入账日期最早的一条用于展示。
///
/// `confirmed_only` 为真时只列本纳税所属期已确认的明细（查询页「仅已确认」）。
pub(crate) fn fetch_review_rows(
    tx: &Transaction<'_>,
    run_id: i64,
    period_from: &str,
    period_to: &str,
    confirmed_only: bool,
) -> Result<Vec<RawReviewRow>, String> {
    let scope = if confirmed_only {
        "cf.id IS NOT NULL AND cf.run_id = ?3"
    } else {
        "(substr(replace(p.booked_date, '-', ''), 1, 6) BETWEEN ?1 AND ?2
          OR r.id IS NOT NULL OR o.id IS NOT NULL)"
    };
    let sql = format!(
        "{BOOKED_CTE},
         picked AS (
             SELECT header_id, voucher_no, booked_date,
                    ROW_NUMBER() OVER (
                        PARTITION BY header_id ORDER BY priority, booked_date
                    ) AS rn
             FROM booked
         )
         SELECT
             d.id, d.header_id, d.line_no,
             substr(replace(p.booked_date, '-', ''), 1, 6), p.voucher_no,
             h.inv_unq_key, h.issue_date,
             s.tax_no, s.name,
             d.goods_name, d.tax_code, d.goods_category, d.tax_rate,
             d.amount, d.tax, d.total, h.inv_type,
             r.id, o.included, o.remark, o.stamp_tax_category_code,
             cf.category_code AS cf_category, cf.base_amount AS cf_base, cf.rate_ppm AS cf_rate,
             cf.stamp_tax AS cf_tax, cf.decision AS cf_decision, {CONFIRM_COLS}
         FROM picked p
         JOIN invoice_headers h  ON h.id = p.header_id
         JOIN sellers s          ON s.id = h.seller_id
         JOIN invoice_details d  ON d.header_id = p.header_id
         LEFT JOIN stamp_tax_results r
                ON r.detail_id = d.id AND r.run_id = ?3
         LEFT JOIN stamp_tax_overrides o
                ON o.detail_id = d.id AND o.run_id = ?3
         LEFT JOIN stamp_tax_confirmed_lines cf ON cf.detail_id = d.id
         LEFT JOIN tax_run_batches b ON b.id = cf.batch_id
         LEFT JOIN tax_runs tr       ON tr.id = cf.run_id
         WHERE p.rn = 1 AND {scope}
         ORDER BY p.booked_date, d.header_id, d.line_no"
    );

    let mut statement = tx
        .prepare(&sql)
        .map_err(|error| format!("准备核对列表查询失败：{error}"))?;

    let rows = statement
        .query_map(params![period_from, period_to, run_id], |row| {
            Ok(RawReviewRow {
                detail_id: row.get(0)?,
                header_id: row.get(1)?,
                line_no: row.get(2)?,
                booked_period: row.get(3)?,
                voucher_no: row.get(4)?,
                invoice_no: row.get(5)?,
                issue_date: row.get(6)?,
                seller_tax_no: row.get(7)?,
                seller_name: row.get(8)?,
                goods_name: row.get(9)?,
                tax_code: row.get(10)?,
                goods_category: row.get(11)?,
                tax_rate: row.get(12)?,
                amount: row.get(13)?,
                tax: row.get(14)?,
                total: row.get(15)?,
                inv_type: row.get(16)?,
                result_id: row.get(17)?,
                override_included: row.get(18)?,
                override_remark: row.get(19)?,
                override_category_code: row.get(20)?,
                confirmed_category: row.get("cf_category")?,
                confirmed_base: row.get("cf_base")?,
                confirmed_rate: row.get("cf_rate")?,
                confirmed_tax: row.get("cf_tax")?,
                confirmed_decision: row.get("cf_decision")?,
                confirm: read_confirm(row)?,
            })
        })
        .map_err(|error| format!("查询核对列表失败：{error}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("读取核对列表失败：{error}"))
}

pub(crate) fn fetch_rule_hits(
    tx: &Transaction<'_>,
    result_ids: &[i64],
) -> Result<HashMap<i64, Vec<i64>>, String> {
    let mut hits: HashMap<i64, Vec<i64>> = HashMap::new();
    if result_ids.is_empty() {
        return Ok(hits);
    }

    // 分块查询，避免触碰 SQLITE_MAX_VARIABLE_NUMBER。
    for chunk in result_ids.chunks(800) {
        let placeholders = chunk.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let sql = format!(
            "SELECT result_id, rule_id FROM stamp_tax_rule_hits
             WHERE result_id IN ({placeholders}) ORDER BY rule_id"
        );

        let mut statement = tx
            .prepare(&sql)
            .map_err(|error| format!("准备命中规则查询失败：{error}"))?;

        let rows = statement
            .query_map(params_from_iter(chunk.iter()), |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
            })
            .map_err(|error| format!("查询命中规则失败：{error}"))?;

        for row in rows {
            let (result_id, rule_id) =
                row.map_err(|error| format!("读取命中规则失败：{error}"))?;
            hits.entry(result_id).or_default().push(rule_id);
        }
    }

    Ok(hits)
}

// ───────────────── 人工结论读写 ─────────────────

/// 写入或更新一条人工结论。两项全空时删除该行，表示恢复系统结论。
pub(crate) fn upsert_override(
    tx: &Transaction<'_>,
    run_id: i64,
    detail_id: i64,
    included: Option<bool>,
    category_code: Option<&str>,
    remark: Option<&str>,
    updated_at: &str,
) -> Result<(), String> {
    let remark = remark.map(str::trim).filter(|value| !value.is_empty());

    // 指定类别即表示计入；明确不计入时丢弃类别。
    let (included, category_code) = match (included, category_code) {
        (Some(false), _) => (Some(false), None),
        (_, Some(code)) => (Some(true), Some(code)),
        (included, None) => (included, None),
    };

    if let Some(code) = category_code {
        let valid = tx
            .query_row(
                "SELECT EXISTS (SELECT 1 FROM stamp_tax_categories WHERE code = ?1)",
                params![code],
                |row| row.get::<_, i64>(0),
            )
            .map(|exists| exists != 0)
            .map_err(|error| format!("校验印花税类别失败：{error}"))?;
        if !valid {
            return Err("所选印花税类别不存在或已停用".to_string());
        }
    }

    if included.is_none() && remark.is_none() {
        tx.execute(
            "DELETE FROM stamp_tax_overrides WHERE detail_id = ?1 AND run_id = ?2",
            params![detail_id, run_id],
        )
        .map_err(|error| format!("清除人工结论失败：{error}"))?;
        return Ok(());
    }

    tx.execute(
        "INSERT INTO stamp_tax_overrides
            (run_id, detail_id, included, stamp_tax_category_code, remark, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(run_id, detail_id) DO UPDATE SET
            included   = excluded.included,
            stamp_tax_category_code = excluded.stamp_tax_category_code,
            remark     = excluded.remark,
            updated_at = excluded.updated_at",
        params![
            run_id,
            detail_id,
            included.map(|value| value as i64),
            category_code,
            remark,
            updated_at
        ],
    )
    .map_err(|error| format!("保存人工结论失败：{error}"))?;

    drop_if_same_as_system(tx, run_id, detail_id)
}

/// 人工结论与系统结论完全一致（计入与否相同、类别等于唯一命中规则的类别或未指定、没有备注）时，
/// 按「没操作过」处理：删掉这一行。
fn drop_if_same_as_system(
    tx: &Transaction<'_>,
    run_id: i64,
    detail_id: i64,
) -> Result<(), String> {
    tx.execute(
        "DELETE FROM stamp_tax_overrides
         WHERE detail_id = ?1 AND run_id = ?2 AND remark IS NULL
           AND (included IS NULL OR included = EXISTS (
                SELECT 1 FROM stamp_tax_results r WHERE r.detail_id = ?1 AND r.run_id = ?2))
           AND (stamp_tax_category_code IS NULL OR stamp_tax_category_code = (
                SELECT MIN(m.stamp_tax_category_code)
                FROM stamp_tax_results r
                JOIN stamp_tax_rule_hits h ON h.result_id = r.id
                JOIN tax_match_rules m ON m.id = h.rule_id
                WHERE r.detail_id = ?1 AND r.run_id = ?2 AND m.stamp_tax_category_code IS NOT NULL
                HAVING COUNT(*) = 1))",
        params![detail_id, run_id],
    )
    .map_err(|error| format!("清理与系统一致的人工结论失败：{error}"))?;
    Ok(())
}

/// 校验 detail_id 能在本纳税所属期修改：凭证入账期间在所选范围内，或本所属期已有结论；
/// 且没有在任何所属期被确认（已确认/已锁定的明细不可改）。
pub(crate) fn detail_editable(
    tx: &Transaction<'_>,
    run_id: i64,
    period_from: &str,
    period_to: &str,
    detail_id: i64,
) -> Result<bool, String> {
    let sql = format!(
        "{BOOKED_CTE}
         SELECT EXISTS (
             SELECT 1 FROM invoice_details d
             WHERE d.id = ?1
               AND (EXISTS (SELECT 1 FROM booked b
                             WHERE b.header_id = d.header_id
                               AND {BOOKED_PERIOD} BETWEEN ?3 AND ?4)
                    OR EXISTS (SELECT 1 FROM stamp_tax_results r
                                WHERE r.detail_id = d.id AND r.run_id = ?2)
                    OR EXISTS (SELECT 1 FROM stamp_tax_overrides o
                                WHERE o.detail_id = d.id AND o.run_id = ?2))
               AND NOT EXISTS (SELECT 1 FROM stamp_tax_confirmed_lines c
                                WHERE c.detail_id = d.id)
         )"
    );

    tx.query_row(
        &sql,
        params![detail_id, run_id, period_from, period_to],
        |row| row.get::<_, i64>(0),
    )
    .map(|exists| exists != 0)
    .map_err(|error| format!("校验发票明细归属失败：{error}"))
}

/// 本纳税所属期里仍须指定印花税类别的行数。确认时用核对行的 needs_category_choice 判定，
/// 这个 SQL 版只在测试里用来校验两种口径一致。
#[cfg(test)]
/// 口径与前端「待选类别」一致：最终计入、且没有人工类别、且（命中多条规则 或 命中规则没有类别）。
/// 已确认的明细（含补充批次里批次 1 的）不算。
pub(crate) fn count_unresolved(tx: &Transaction<'_>, run_id: i64) -> Result<usize, String> {
    tx.query_row(
        "SELECT COUNT(*) FROM (
             SELECT d.id
             FROM invoice_details d
             LEFT JOIN stamp_tax_results r
                    ON r.detail_id = d.id AND r.run_id = ?1
             LEFT JOIN stamp_tax_overrides o
                    ON o.detail_id = d.id AND o.run_id = ?1
             WHERE (r.id IS NOT NULL OR o.id IS NOT NULL)
               AND COALESCE(o.included, 1) = 1
               AND o.stamp_tax_category_code IS NULL
               AND NOT EXISTS (SELECT 1 FROM stamp_tax_confirmed_lines c WHERE c.detail_id = d.id)
               AND (SELECT COUNT(*) FROM stamp_tax_rule_hits x
                      JOIN tax_match_rules ru ON ru.id = x.rule_id
                     WHERE x.result_id = r.id
                       AND ru.stamp_tax_category_code IS NOT NULL) <> 1
         )",
        params![run_id],
        |row| row.get::<_, i64>(0),
    )
    .map(|count| count as usize)
    .map_err(|error| format!("统计待选类别失败：{error}"))
}

pub(crate) fn fetch_categories(
    tx: &Transaction<'_>,
) -> Result<HashMap<String, (String, Option<i64>)>, String> {
    let mut statement = tx
        .prepare("SELECT code, name, rate_ppm FROM stamp_tax_categories")
        .map_err(|error| format!("准备印花税类别查询失败：{error}"))?;

    let rows = statement
        .query_map([], |row| Ok((row.get(0)?, (row.get(1)?, row.get(2)?))))
        .map_err(|error| format!("查询印花税类别失败：{error}"))?;

    rows.collect::<Result<HashMap<_, _>, _>>()
        .map_err(|error| format!("读取印花税类别失败：{error}"))
}

// ───────────────── 确认快照 ─────────────────

pub(crate) struct ConfirmLine {
    pub detail_id: i64,
    pub header_id: i64,
    pub category_code: String,
    pub base_amount: i64,
    pub rate_ppm: i64,
    pub stamp_tax: i64,
    pub decision: &'static str,
    pub remark: Option<String>,
}

pub(crate) fn insert_confirmed_line(
    tx: &Transaction<'_>,
    run_id: i64,
    batch_id: i64,
    line: &ConfirmLine,
    confirmed_at: &str,
) -> Result<(), String> {
    tx.execute(
        "INSERT INTO stamp_tax_confirmed_lines
            (run_id, batch_id, detail_id, header_id, category_code, base_amount, rate_ppm,
             stamp_tax, decision, remark, confirmed_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            run_id,
            batch_id,
            line.detail_id,
            line.header_id,
            line.category_code,
            line.base_amount,
            line.rate_ppm,
            line.stamp_tax,
            line.decision,
            line.remark,
            confirmed_at
        ],
    )
    .map_err(|error| format!("保存已确认明细失败：{error}"))?;
    Ok(())
}


// ───────────────── 采集表汇总 ─────────────────

/// 采集表的一行：按 (类别, 税率) 汇总后的计税金额。税目 / 子目由类别表的父子关系拆出。
pub(crate) struct CollectionLine {
    pub item_code: String,
    pub item_name: String,
    pub sub_item_code: Option<String>,
    pub sub_item_name: Option<String>,
    pub rate_ppm: i64,
    /// 计税金额（分），已确认明细的 base_amount 之和
    pub taxable_fen: i64,
}

/// 某批次已确认明细按 (类别, 税率) 汇总。类别有父级的是子目（父级是税目），否则类别本身就是税目。
pub(crate) fn collection_lines(
    tx: &Transaction<'_>,
    batch_id: i64,
) -> Result<Vec<CollectionLine>, String> {
    let mut statement = tx
        .prepare(
            "SELECT c.code, c.name, p.code, p.name, cl.rate_ppm, SUM(cl.base_amount)
             FROM stamp_tax_confirmed_lines cl
             JOIN stamp_tax_categories c ON c.code = cl.category_code
             LEFT JOIN stamp_tax_categories p ON p.code = c.parent_code
             WHERE cl.batch_id = ?1
             GROUP BY c.code, cl.rate_ppm
             ORDER BY c.code, cl.rate_ppm",
        )
        .map_err(|error| format!("准备采集表汇总失败：{error}"))?;
    let rows = statement
        .query_map(params![batch_id], |row| {
            let (code, name): (String, String) = (row.get(0)?, row.get(1)?);
            let (parent_code, parent_name): (Option<String>, Option<String>) = (row.get(2)?, row.get(3)?);
            // 有父级：父级是税目、本类别是子目；没有父级：本类别就是税目
            let (item_code, item_name, sub_item_code, sub_item_name) = match (parent_code, parent_name) {
                (Some(pc), Some(pn)) => (pc, pn, Some(code), Some(name)),
                _ => (code, name, None, None),
            };
            Ok(CollectionLine {
                item_code,
                item_name,
                sub_item_code,
                sub_item_name,
                rate_ppm: row.get(4)?,
                taxable_fen: row.get(5)?,
            })
        })
        .map_err(|error| format!("查询采集表汇总失败：{error}"))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("读取采集表汇总失败：{error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    /// 三张发票各一条明细，都用 Bank 凭证入账：
    /// 1 → 2026-09-05，2 → 2026-09-28，3 → 2026-10-03。
    /// 规则 900/901 都带类别，用来构造「命中一条」和「命中多条」。
    /// run 1 = 纳税所属期 202609，run 2 = 202610。
    fn seeded() -> Connection {
        let mut conn = Connection::open_in_memory().unwrap();
        // init_db 建 FTS5 表需要自定义分词器 simple
        unsafe {
            conn.load_extension_enable().unwrap();
            let dll = concat!(env!("CARGO_MANIFEST_DIR"), "/resources/simple.dll");
            conn.load_extension(dll, None::<&str>).unwrap();
        }
        crate::db::schema::init_db(&mut conn).unwrap();
        conn.execute_batch(
            "INSERT INTO sellers(id, tax_no, name) VALUES (1, 'T1', '销方甲');
             INSERT INTO invoice_headers(id, e_num, seller_id, issue_date, amount, tax, total, source, inv_type, state, is_pos, risk)
               VALUES (1, 'E1', 1, '2026-09-01', 100, 9, 109, 1, 1, 1, 1, 1),
                      (2, 'E2', 1, '2026-09-01', 100, 9, 109, 1, 1, 1, 1, 1),
                      (3, 'E3', 1, '2026-10-01', 100, 9, 109, 1, 1, 1, 1, 1);
             INSERT INTO invoice_details(id, header_id, line_no, goods_name, amount, tax_rate, tax, total)
               VALUES (1, 1, 1, '甲', 100, '9%', 9, 109), (2, 2, 1, '乙', 100, '9%', 9, 109), (3, 3, 1, '丙', 100, '9%', 9, 109);
             INSERT INTO bank_invoice_vouchers(header_id, voucher_no, posted_date, received_date)
               VALUES (1, 'B1', '2026-09-05', '2026-09-05'), (2, 'B2', '2026-09-28', '2026-09-28'), (3, 'B3', '2026-10-03', '2026-10-03');
             INSERT INTO stamp_tax_categories(code, name, parent_code, rate_ppm) VALUES ('900', '甲类', NULL, 300), ('901', '乙类', NULL, 50);
             INSERT INTO tax_match_rules(id, scope, rule_category, stamp_tax_category_code, tax_name_match_type, tax_name_values, source, created_at, rule_signature)
               VALUES (900, 'stamp_tax', 'r1', '900', 'eq', 'x', 'manual', '2026-09-01', randomblob(32)),
                      (901, 'stamp_tax', 'r2', '901', 'eq', 'y', 'manual', '2026-09-01', randomblob(32));
             INSERT INTO tax_runs(id, module, filing_from, filing_to, src_from, src_to, created_by, created_at)
               VALUES (1, 'stamp_tax', '202609', '202609', '202609', '202610', 'u', 't'),
                      (2, 'stamp_tax', '202610', '202610', '202609', '202610', 'u', 't');
             INSERT INTO tax_run_batches(id, run_id, batch_no, created_by, created_at) VALUES (1, 1, 1, 'u', 't'), (2, 2, 1, 'u', 't');",
        )
        .unwrap();
        conn
    }

    fn candidate(id: i64) -> MatchCandidate {
        MatchCandidate {
            detail_id: id,
            header_id: id,
            seller_tax_no: "T1".into(),
            tax_code: None,
            goods_category: None,
        }
    }

    fn confirm(tx: &Transaction<'_>, run: i64, batch: i64, detail: i64) {
        insert_confirmed_line(
            tx,
            run,
            batch,
            &ConfirmLine {
                detail_id: detail,
                header_id: detail,
                category_code: "900".into(),
                base_amount: 100,
                rate_ppm: 300,
                stamp_tax: 0,
                decision: "auto",
                remark: None,
            },
            "now",
        )
        .unwrap();
    }

    fn ids(tx: &Transaction<'_>, from: &str, to: &str) -> Vec<i64> {
        let mut v: Vec<i64> = fetch_match_candidates(tx, from, to)
            .unwrap()
            .iter()
            .map(|c| c.detail_id)
            .collect();
        v.sort();
        v
    }

    #[test]
    fn candidates_filter_by_booked_period_range_inclusive() {
        let mut conn = seeded();
        let tx = conn.transaction().unwrap();
        assert_eq!(ids(&tx, "202609", "202609"), vec![1, 2]);
        assert_eq!(ids(&tx, "202610", "202610"), vec![3]);
        assert_eq!(ids(&tx, "202609", "202610"), vec![1, 2, 3]);
        assert_eq!(ids(&tx, "202611", "202612"), Vec::<i64>::new());
    }

    #[test]
    fn only_confirmed_details_are_excluded_from_other_runs() {
        let mut conn = seeded();
        let tx = conn.transaction().unwrap();
        // 202609 里明细 1 机器命中、明细 2 人工计入（都没确认）：别的所属期仍可匹配它们
        upsert_result(&tx, &candidate(1), 1, "now", &[900]).unwrap();
        upsert_override(&tx, 1, 2, Some(true), None, None, "now").unwrap();
        assert_eq!(ids(&tx, "202609", "202610"), vec![1, 2, 3]);

        // 明细 1 确认后，不再是候选；取消确认（删快照）后又回来
        confirm(&tx, 1, 1, 1);
        assert_eq!(ids(&tx, "202609", "202610"), vec![2, 3]);
        tx.execute("DELETE FROM stamp_tax_confirmed_lines", []).unwrap();
        assert_eq!(ids(&tx, "202609", "202610"), vec![1, 2, 3]);
    }

    #[test]
    fn same_detail_can_have_results_in_two_runs() {
        let mut conn = seeded();
        let tx = conn.transaction().unwrap();
        upsert_result(&tx, &candidate(1), 1, "t1", &[900]).unwrap();
        upsert_result(&tx, &candidate(1), 2, "t1", &[901]).unwrap();
        let n: i64 = tx
            .query_row("SELECT COUNT(*) FROM stamp_tax_results WHERE detail_id = 1", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 2);
        upsert_override(&tx, 1, 1, Some(false), None, None, "t").unwrap();
        upsert_override(&tx, 2, 1, None, None, Some("备注"), "t").unwrap();
        assert_eq!(count_overrides(&tx, 1).unwrap(), 1);
        assert_eq!(count_overrides(&tx, 2).unwrap(), 1);
    }

    #[test]
    fn review_keeps_saved_rows_when_source_range_changes() {
        let mut conn = seeded();
        let tx = conn.transaction().unwrap();
        upsert_result(&tx, &candidate(1), 1, "now", &[900]).unwrap();

        // 来源范围换成完全不含明细 1 的区间，本期已入库的行仍然在；范围内的明细 3 也在
        let rows = fetch_review_rows(&tx, 1, "202610", "202610", false).unwrap();
        let got: Vec<i64> = rows.iter().map(|r| r.detail_id).collect();
        assert!(got.contains(&1));
        assert!(got.contains(&3));
        assert!(!got.contains(&2));
        // 展示的期间列是入账月份
        assert_eq!(rows.iter().find(|r| r.detail_id == 1).unwrap().booked_period, "202609");
    }

    #[test]
    fn confirmed_rows_carry_batch_and_voucher_info_across_runs() {
        let mut conn = seeded();
        let tx = conn.transaction().unwrap();
        confirm(&tx, 1, 1, 1);
        tx.execute(
            "UPDATE tax_run_batches SET confirmed_by='u', confirmed_at='c', locked_by='u', locked_at='l',
                    voucher_no='JV01', posted_date='2026-10-05', preparer='张三' WHERE id = 1",
            [],
        )
        .unwrap();
        let rows = fetch_review_rows(&tx, 2, "202609", "202610", false).unwrap();
        let r1 = rows.iter().find(|r| r.detail_id == 1).unwrap();
        let c = r1.confirm.as_ref().unwrap();
        assert_eq!((c.run_id, c.batch_no, c.voucher_no.as_deref()), (1, 1, Some("JV01")));
        assert_eq!(r1.confirmed_category.as_deref(), Some("900"));
        assert!(fetch_review_rows(&tx, 2, "202609", "202610", true).unwrap().is_empty());
        assert_eq!(fetch_review_rows(&tx, 1, "202609", "202610", true).unwrap().len(), 1);
    }

    #[test]
    fn editable_only_when_in_range_or_stored_and_not_confirmed() {
        let mut conn = seeded();
        let tx = conn.transaction().unwrap();
        assert!(!detail_editable(&tx, 1, "202609", "202609", 3).unwrap());
        assert!(detail_editable(&tx, 2, "202610", "202610", 3).unwrap());
        upsert_override(&tx, 1, 3, Some(true), None, None, "t").unwrap();
        assert!(detail_editable(&tx, 1, "202609", "202609", 3).unwrap());
        confirm(&tx, 2, 2, 3);
        assert!(!detail_editable(&tx, 1, "202609", "202609", 3).unwrap());
        assert!(!detail_editable(&tx, 2, "202610", "202610", 3).unwrap());
    }

    #[test]
    fn override_equal_to_system_is_dropped() {
        let mut conn = seeded();
        let tx = conn.transaction().unwrap();
        upsert_result(&tx, &candidate(1), 1, "now", &[900]).unwrap();
        upsert_override(&tx, 1, 1, Some(true), None, None, "now").unwrap();
        assert_eq!(count_overrides(&tx, 1).unwrap(), 0);
        upsert_override(&tx, 1, 1, Some(false), None, None, "now").unwrap();
        assert_eq!(count_overrides(&tx, 1).unwrap(), 1);
        upsert_override(&tx, 1, 1, Some(true), None, None, "now").unwrap();
        assert_eq!(count_overrides(&tx, 1).unwrap(), 0);
        upsert_override(&tx, 1, 1, Some(true), None, Some("备注"), "now").unwrap();
        assert_eq!(count_overrides(&tx, 1).unwrap(), 1);
    }

    #[test]
    fn rematch_with_same_result_keeps_matched_at() {
        let mut conn = seeded();
        let tx = conn.transaction().unwrap();
        let at = |tx: &Transaction<'_>| -> String {
            tx.query_row("SELECT matched_at FROM stamp_tax_results WHERE detail_id = 1", [], |r| r.get(0)).unwrap()
        };
        upsert_result(&tx, &candidate(1), 1, "t1", &[]).unwrap();
        upsert_result(&tx, &candidate(1), 1, "t2", &[]).unwrap();
        assert_eq!(at(&tx), "t1");
        upsert_result(&tx, &candidate(1), 1, "t3", &[900]).unwrap();
        assert_eq!(at(&tx), "t3");
        upsert_result(&tx, &candidate(1), 1, "t4", &[900]).unwrap();
        assert_eq!(at(&tx), "t3");
    }

    #[test]
    fn unresolved_matches_the_review_row_rule() {
        let mut conn = seeded();
        let tx = conn.transaction().unwrap();
        upsert_result(&tx, &candidate(1), 1, "now", &[900]).unwrap();
        assert_eq!(count_unresolved(&tx, 1).unwrap(), 0);

        upsert_result(&tx, &candidate(2), 1, "now", &[900, 901]).unwrap();
        assert_eq!(count_unresolved(&tx, 1).unwrap(), 1);

        upsert_override(&tx, 1, 2, None, Some("901"), None, "now").unwrap();
        assert_eq!(count_unresolved(&tx, 1).unwrap(), 0);

        upsert_override(&tx, 1, 1, Some(false), None, None, "now").unwrap();
        assert_eq!(count_unresolved(&tx, 1).unwrap(), 0);

        upsert_override(&tx, 1, 3, Some(true), None, None, "now").unwrap();
        assert_eq!(count_unresolved(&tx, 1).unwrap(), 1);

        // 别的纳税所属期不受影响；已确认的行不再算待选
        assert_eq!(count_unresolved(&tx, 2).unwrap(), 0);
        confirm(&tx, 1, 1, 3);
        assert_eq!(count_unresolved(&tx, 1).unwrap(), 0);
    }

    /// 差旅发票在 posted_date 为空时回退到 check_time（税局导出的原样文本，没有日期规整）。
    /// 斜杠日期规整成横线后，入账期间照样取对。
    fn travel_candidates(check_time: &str) -> Vec<i64> {
        let mut conn = Connection::open_in_memory().unwrap();
        unsafe {
            conn.load_extension_enable().unwrap();
            let dll = concat!(env!("CARGO_MANIFEST_DIR"), "/resources/simple.dll");
            conn.load_extension(dll, None::<&str>).unwrap();
        }
        crate::db::schema::init_db(&mut conn).unwrap();
        conn.execute_batch(
            "INSERT INTO sellers(id, tax_no, name) VALUES (1, 'T1', '甲');
             INSERT INTO invoice_headers(id, e_num, seller_id, issue_date, amount, tax, total, source, inv_type, state, is_pos, risk)
               VALUES (1,'E1',1,'2026-09-01',100,9,109,1,1,1,1,1);
             INSERT INTO invoice_details(id, header_id, line_no, goods_name, amount, tax_rate, tax, total)
               VALUES (1,1,1,'a',100,'9%',9,109);",
        )
        .unwrap();
        conn.execute(
            "INSERT INTO certified_list(header_id, valid_deduct_amount, invoice_tag, check_time, risk_status, tax_period, voucher_no, is_travel_expense)
             VALUES (1, 9, 't', ?1, 0, '202609', 'V1', 1)",
            [check_time],
        )
        .unwrap();
        let tx = conn.transaction().unwrap();
        fetch_match_candidates(&tx, "202609", "202609")
            .unwrap()
            .iter()
            .map(|c| c.detail_id)
            .collect()
    }

    #[test]
    fn dash_slash_and_datetime_travel_dates_are_matched() {
        assert_eq!(travel_candidates("2026-09-08"), vec![1]);
        assert_eq!(travel_candidates("2026-09-08 14:30:00"), vec![1]);
        assert_eq!(travel_candidates("2026/09/08"), vec![1]);
    }
}
