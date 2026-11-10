use std::collections::HashMap;

use rusqlite::{params, params_from_iter, Transaction};

use super::models::{MatchCandidate, RawReviewRow, RuleForMatch};

// ───────────────── 做账期间判定 ─────────────────
//
// 印花税按「做账期间」（凭证入账/过账日期所在月份）判定，而非认证清单的认证期间——
// 很多发票不走增值税认证，但照样在 AP/Bank/IC/差旅 四张凭证表之一做账，必须纳入印花税考量。
// 一张发票只要在任一凭证表里有凭证号，就算做账；取号时按 AP > Bank > IC > 差旅 优先级，
// 多个来源都命中时取优先级最高且月份最早的一条展示。
//
// 四个来源的日期字段不保证都非空（历史数据/旧版导入），各自退化到一个必然非空的日期列：
//   AP   entry_date  → received_date
//   Bank posted_date（建表时已 NOT NULL）
//   IC   posted_date → received_date
//   差旅 posted_date → check_time（认证清单必填）
const BOOKED_CTE: &str = "
WITH booked AS (
    SELECT v.header_id AS header_id, v.voucher_no AS voucher_no,
           substr(COALESCE(v.entry_date, v.received_date), 1, 4)
               || substr(COALESCE(v.entry_date, v.received_date), 6, 2) AS booked_period,
           1 AS priority
    FROM ap_invoice_vouchers v
    WHERE v.header_id IS NOT NULL
      AND TRIM(COALESCE(v.voucher_no, '')) <> ''
      AND COALESCE(v.entry_date, v.received_date) IS NOT NULL

    UNION ALL

    SELECT v.header_id, v.voucher_no,
           substr(v.posted_date, 1, 4) || substr(v.posted_date, 6, 2),
           2
    FROM bank_invoice_vouchers v
    WHERE TRIM(COALESCE(v.voucher_no, '')) <> ''
      AND v.posted_date IS NOT NULL

    UNION ALL

    SELECT v.header_id, v.document_no,
           substr(COALESCE(v.posted_date, v.received_date), 1, 4)
               || substr(COALESCE(v.posted_date, v.received_date), 6, 2),
           3
    FROM ic_vouchers v
    WHERE TRIM(COALESCE(v.document_no, '')) <> ''
      AND COALESCE(v.posted_date, v.received_date) IS NOT NULL

    UNION ALL

    SELECT cl.header_id, cl.voucher_no,
           substr(COALESCE(cl.posted_date, cl.check_time), 1, 4)
               || substr(COALESCE(cl.posted_date, cl.check_time), 6, 2),
           4
    FROM certified_list cl
    WHERE COALESCE(cl.is_travel_expense, 0) = 1
      AND TRIM(COALESCE(cl.voucher_no, '')) <> ''
      AND COALESCE(cl.posted_date, cl.check_time) IS NOT NULL
)
";

// ───────────────── 匹配候选 ─────────────────

/// 取出期间内「已做账」的发票明细：header 在任一凭证表命中、且做账月份落在所选期间。
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
         WHERE b.booked_period BETWEEN ?1 AND ?2"
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
        "SELECT r.id, r.rule_category, r.stamp_tax_category_id, c.category_name, c.rate_bp, r.note, r.needs_manual_review, r.enabled,
                r.seller_tax_id_match_type, r.seller_tax_id_values,
                r.tax_code_match_type, r.tax_code_values,
                r.tax_name_match_type, r.tax_name_values
         FROM tax_match_rules r
         LEFT JOIN stamp_tax_categories c ON c.id = r.stamp_tax_category_id
         WHERE r.scope = 'stamp_tax' AND r.enabled = 1
         ORDER BY r.id"
    } else {
        "SELECT r.id, r.rule_category, r.stamp_tax_category_id, c.category_name, c.rate_bp, r.note, r.needs_manual_review, r.enabled,
                r.seller_tax_id_match_type, r.seller_tax_id_values,
                r.tax_code_match_type, r.tax_code_values,
                r.tax_name_match_type, r.tax_name_values
         FROM tax_match_rules r
         LEFT JOIN stamp_tax_categories c ON c.id = r.stamp_tax_category_id
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
                stamp_tax_category_id: row.get(2)?,
                stamp_tax_category: row.get(3)?,
                rate_bp: row.get(4)?,
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

/// 清空同一期间的系统结果；rule_hits 由 ON DELETE CASCADE 连带清除。
/// 不触碰 stamp_tax_overrides。
pub(crate) fn clear_results(
    tx: &Transaction<'_>,
    period_from: &str,
    period_to: &str,
) -> Result<(), String> {
    tx.execute(
        "DELETE FROM stamp_tax_results
         WHERE period_from = ?1 AND period_to = ?2",
        params![period_from, period_to],
    )
    .map_err(|error| format!("清除旧匹配结果失败：{error}"))?;
    Ok(())
}

pub(crate) fn insert_result(
    tx: &Transaction<'_>,
    candidate: &MatchCandidate,
    period_from: &str,
    period_to: &str,
    matched_at: &str,
    rule_ids: &[i64],
) -> Result<(), String> {
    tx.execute(
        "INSERT INTO stamp_tax_results
            (detail_id, header_id, period_from, period_to, matched_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            candidate.detail_id,
            candidate.header_id,
            period_from,
            period_to,
            matched_at
        ],
    )
    .map_err(|error| format!("保存匹配结果失败：{error}"))?;

    let result_id = tx.last_insert_rowid();

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

/// 统计该期间已有的人工结论条数，向用户说明有多少条不受重算影响。
pub(crate) fn count_overrides(
    tx: &Transaction<'_>,
    period_from: &str,
    period_to: &str,
) -> Result<usize, String> {
    tx.query_row(
        "SELECT COUNT(*) FROM stamp_tax_overrides
         WHERE period_from = ?1 AND period_to = ?2",
        params![period_from, period_to],
        |row| row.get::<_, i64>(0),
    )
    .map(|count| count as usize)
    .map_err(|error| format!("统计人工结论失败：{error}"))
}

// ───────────────── 核对列表主干查询 ─────────────────

/// 以「已做账」的发票明细为起点，展开全部明细行；命中结果与人工结论用 LEFT JOIN 附加。
/// 一张发票可能同时被多张凭证表命中（极少见），按 AP > Bank > IC > 差旅 优先级去重，
/// 取做账月份最早的一条用于展示。
pub(crate) fn fetch_review_rows(
    tx: &Transaction<'_>,
    period_from: &str,
    period_to: &str,
) -> Result<Vec<RawReviewRow>, String> {
    let sql = format!(
        "{BOOKED_CTE},
         picked AS (
             SELECT header_id, voucher_no, booked_period,
                    ROW_NUMBER() OVER (
                        PARTITION BY header_id ORDER BY priority, booked_period
                    ) AS rn
             FROM booked
             WHERE booked_period BETWEEN ?1 AND ?2
         )
         SELECT
             d.id, d.header_id, d.line_no,
             p.booked_period, p.voucher_no,
             h.inv_unq_key, h.issue_date,
             s.tax_no, s.name,
             d.goods_name, d.tax_code, d.goods_category, d.tax_rate,
             d.amount, d.tax, d.total,
             r.id, o.included, o.remark, o.stamp_tax_category_id
         FROM picked p
         JOIN invoice_headers h  ON h.id = p.header_id
         JOIN sellers s          ON s.id = h.seller_id
         JOIN invoice_details d  ON d.header_id = p.header_id
         LEFT JOIN stamp_tax_results r
                ON r.detail_id = d.id
               AND r.period_from = ?1 AND r.period_to = ?2
         LEFT JOIN stamp_tax_overrides o
                ON o.detail_id = d.id
               AND o.period_from = ?1 AND o.period_to = ?2
         WHERE p.rn = 1
         ORDER BY p.booked_period, d.header_id, d.line_no"
    );

    let mut statement = tx
        .prepare(&sql)
        .map_err(|error| format!("准备核对列表查询失败：{error}"))?;

    let rows = statement
        .query_map(params![period_from, period_to], |row| {
            Ok(RawReviewRow {
                detail_id: row.get(0)?,
                header_id: row.get(1)?,
                line_no: row.get(2)?,
                tax_period: row.get(3)?,
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
                result_id: row.get(16)?,
                override_included: row.get(17)?,
                override_remark: row.get(18)?,
                override_category_id: row.get(19)?,
            })
        })
        .map_err(|error| format!("查询核对列表失败：{error}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("读取核对列表失败：{error}"))
}

/// 按 result_id 取出命中的 rule_id 列表。
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
    period_from: &str,
    period_to: &str,
    detail_id: i64,
    included: Option<bool>,
    category_id: Option<i64>,
    remark: Option<&str>,
    updated_at: &str,
) -> Result<(), String> {
    let remark = remark.map(str::trim).filter(|value| !value.is_empty());

    // 指定类别即表示计入；明确不计入时丢弃类别。
    let (included, category_id) = match (included, category_id) {
        (Some(false), _) => (Some(false), None),
        (_, Some(id)) => (Some(true), Some(id)),
        (included, None) => (included, None),
    };

    if let Some(id) = category_id {
        let valid = tx
            .query_row(
                "SELECT EXISTS (SELECT 1 FROM stamp_tax_categories WHERE id = ?1 AND enabled = 1)",
                params![id],
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
            "DELETE FROM stamp_tax_overrides
             WHERE detail_id = ?1 AND period_from = ?2 AND period_to = ?3",
            params![detail_id, period_from, period_to],
        )
        .map_err(|error| format!("清除人工结论失败：{error}"))?;
        return Ok(());
    }

    tx.execute(
        "INSERT INTO stamp_tax_overrides
            (detail_id, period_from, period_to, included, stamp_tax_category_id, remark, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(detail_id, period_from, period_to) DO UPDATE SET
            included   = excluded.included,
            stamp_tax_category_id = excluded.stamp_tax_category_id,
            remark     = excluded.remark,
            updated_at = excluded.updated_at",
        params![
            detail_id,
            period_from,
            period_to,
            included.map(|value| value as i64),
            category_id,
            remark,
            updated_at
        ],
    )
    .map_err(|error| format!("保存人工结论失败：{error}"))?;

    Ok(())
}

/// 校验 detail_id 确实属于所选期间内「已做账」的发票，防止前端传入越界数据。
pub(crate) fn detail_in_period(
    tx: &Transaction<'_>,
    period_from: &str,
    period_to: &str,
    detail_id: i64,
) -> Result<bool, String> {
    let sql = format!(
        "{BOOKED_CTE}
         SELECT EXISTS (
             SELECT 1 FROM invoice_details d
             JOIN booked b ON b.header_id = d.header_id
             WHERE d.id = ?3 AND b.booked_period BETWEEN ?1 AND ?2
         )"
    );

    tx.query_row(
        &sql,
        params![period_from, period_to, detail_id],
        |row| row.get::<_, i64>(0),
    )
    .map(|exists| exists != 0)
    .map_err(|error| format!("校验发票明细归属失败：{error}"))
}

/// 全部印花税类别（含已停用，历史人工指定仍需显示）：id → (名称, 税率基点)。
pub(crate) fn fetch_categories(
    tx: &Transaction<'_>,
) -> Result<HashMap<i64, (String, i64)>, String> {
    let mut statement = tx
        .prepare("SELECT id, category_name, rate_bp FROM stamp_tax_categories")
        .map_err(|error| format!("准备印花税类别查询失败：{error}"))?;

    let rows = statement
        .query_map([], |row| Ok((row.get(0)?, (row.get(1)?, row.get(2)?))))
        .map_err(|error| format!("查询印花税类别失败：{error}"))?;

    rows.collect::<Result<HashMap<_, _>, _>>()
        .map_err(|error| format!("读取印花税类别失败：{error}"))
}
