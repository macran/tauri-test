use std::collections::HashMap;

use rusqlite::{params, params_from_iter, Transaction};

use super::models::{MatchCandidate, RawReviewRow, RuleForMatch};

// ───────────────── 凭证号回填 ─────────────────

/// 按期间把 AP / BANK / IC 的凭证号回填到 certified_list.voucher_no。
/// is_travel_expense = 1 的行由导入流程写入，这里不覆盖；只填当前为空的行。
pub(crate) fn backfill_voucher_no(
    tx: &Transaction<'_>,
    period_from: &str,
    period_to: &str,
) -> Result<usize, String> {
    let affected = tx
        .execute(
            "UPDATE certified_list SET voucher_no = COALESCE(
                (SELECT v.voucher_no FROM ap_invoice_vouchers v
                  WHERE v.header_id = certified_list.header_id
                    AND TRIM(COALESCE(v.voucher_no, '')) <> '' LIMIT 1),
                (SELECT v.voucher_no FROM bank_invoice_vouchers v
                  WHERE v.header_id = certified_list.header_id
                    AND TRIM(COALESCE(v.voucher_no, '')) <> '' LIMIT 1),
                (SELECT v.document_no FROM ic_vouchers v
                  WHERE v.header_id = certified_list.header_id
                    AND TRIM(COALESCE(v.document_no, '')) <> '' LIMIT 1)
             )
             WHERE tax_period BETWEEN ?1 AND ?2
               AND COALESCE(is_travel_expense, 0) = 0
               AND TRIM(COALESCE(voucher_no, '')) = ''
               AND EXISTS (
                   SELECT 1 FROM ap_invoice_vouchers v
                    WHERE v.header_id = certified_list.header_id
                      AND TRIM(COALESCE(v.voucher_no, '')) <> ''
                   UNION ALL
                   SELECT 1 FROM bank_invoice_vouchers v
                    WHERE v.header_id = certified_list.header_id
                      AND TRIM(COALESCE(v.voucher_no, '')) <> ''
                   UNION ALL
                   SELECT 1 FROM ic_vouchers v
                    WHERE v.header_id = certified_list.header_id
                      AND TRIM(COALESCE(v.document_no, '')) <> ''
               )",
            params![period_from, period_to],
        )
        .map_err(|error| format!("回填凭证号失败：{error}"))?;

    Ok(affected)
}

// ───────────────── 匹配候选 ─────────────────

/// 取出期间内「有凭证号」的发票明细。无凭证号的一律不参与匹配。
pub(crate) fn fetch_match_candidates(
    tx: &Transaction<'_>,
    period_from: &str,
    period_to: &str,
) -> Result<Vec<MatchCandidate>, String> {
    let mut statement = tx
        .prepare(
            "SELECT d.id, d.header_id, s.tax_no, d.tax_code, d.goods_category
             FROM certified_list cl
             JOIN invoice_headers h  ON h.id = cl.header_id
             JOIN sellers s          ON s.id = h.seller_id
             JOIN invoice_details d  ON d.header_id = cl.header_id
             WHERE cl.tax_period BETWEEN ?1 AND ?2
               AND TRIM(COALESCE(cl.voucher_no, '')) <> ''",
        )
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
        "SELECT id, rule_category, note, needs_manual_review, enabled,
                seller_tax_id_match_type, seller_tax_id_values,
                tax_code_match_type, tax_code_values,
                tax_name_match_type, tax_name_values
         FROM tax_match_rules
         WHERE scope = 'input_tax_transfer' AND enabled = 1
         ORDER BY id"
    } else {
        "SELECT id, rule_category, note, needs_manual_review, enabled,
                seller_tax_id_match_type, seller_tax_id_values,
                tax_code_match_type, tax_code_values,
                tax_name_match_type, tax_name_values
         FROM tax_match_rules
         WHERE scope = 'input_tax_transfer'
         ORDER BY id"
    };

    let mut statement = tx
        .prepare(sql)
        .map_err(|error| format!("准备规则查询失败：{error}"))?;

    let rows = statement
        .query_map([], |row| {
            Ok(RuleForMatch {
                id: row.get(0)?,
                rule_category: row.get(1)?,
                note: row.get(2)?,
                needs_manual_review: row.get::<_, i64>(3)? != 0,
                enabled: row.get::<_, i64>(4)? != 0,
                seller_tax_id_match_type: row.get(5)?,
                seller_tax_id_values: row.get(6)?,
                tax_code_match_type: row.get(7)?,
                tax_code_values: row.get(8)?,
                tax_name_match_type: row.get(9)?,
                tax_name_values: row.get(10)?,
            })
        })
        .map_err(|error| format!("查询规则失败：{error}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("读取规则失败：{error}"))
}

// ───────────────── 系统结果写入 ─────────────────

/// 清空同一期间的系统结果；rule_hits 由 ON DELETE CASCADE 连带清除。
/// 不触碰 welfare_transfer_overrides。
pub(crate) fn clear_results(
    tx: &Transaction<'_>,
    period_from: &str,
    period_to: &str,
) -> Result<(), String> {
    tx.execute(
        "DELETE FROM welfare_transfer_results
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
        "INSERT INTO welfare_transfer_results
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
        .prepare_cached(
            "INSERT INTO welfare_transfer_rule_hits (result_id, rule_id) VALUES (?1, ?2)",
        )
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
        "SELECT COUNT(*) FROM welfare_transfer_overrides
         WHERE period_from = ?1 AND period_to = ?2",
        params![period_from, period_to],
        |row| row.get::<_, i64>(0),
    )
    .map(|count| count as usize)
    .map_err(|error| format!("统计人工结论失败：{error}"))
}

// ───────────────── 核对列表主干查询 ─────────────────

/// 以 certified_list 的 header_id 为起点展开全部发票明细行。
/// 无凭证号、未命中规则的行同样返回；命中结果与人工结论用 LEFT JOIN 附加。
pub(crate) fn fetch_review_rows(
    tx: &Transaction<'_>,
    period_from: &str,
    period_to: &str,
) -> Result<Vec<RawReviewRow>, String> {
    let mut statement = tx
        .prepare(
            "SELECT
                d.id, d.header_id, d.line_no,
                cl.tax_period,
                CASE WHEN TRIM(COALESCE(cl.voucher_no, '')) = ''
                     THEN NULL ELSE cl.voucher_no END,
                h.inv_unq_key, h.issue_date,
                s.tax_no, s.name,
                d.goods_name, d.tax_code, d.goods_category, d.tax_rate,
                d.amount, d.tax, d.total,
                r.id, o.included, o.transfer_tax, o.remark
             FROM certified_list cl
             JOIN invoice_headers h ON h.id = cl.header_id
             JOIN sellers s         ON s.id = h.seller_id
             JOIN invoice_details d ON d.header_id = cl.header_id
             LEFT JOIN welfare_transfer_results r
                    ON r.detail_id = d.id
                   AND r.period_from = ?1 AND r.period_to = ?2
             LEFT JOIN welfare_transfer_overrides o
                    ON o.detail_id = d.id
                   AND o.period_from = ?1 AND o.period_to = ?2
             WHERE cl.tax_period BETWEEN ?1 AND ?2
             ORDER BY cl.tax_period, d.header_id, d.line_no",
        )
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
                override_transfer_tax: row.get(18)?,
                override_remark: row.get(19)?,
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
            "SELECT result_id, rule_id FROM welfare_transfer_rule_hits
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

/// 写入或更新一条人工结论。三项全空时删除该行，表示恢复系统结论。
pub(crate) fn upsert_override(
    tx: &Transaction<'_>,
    period_from: &str,
    period_to: &str,
    detail_id: i64,
    included: Option<bool>,
    transfer_tax: Option<i64>,
    remark: Option<&str>,
    updated_at: &str,
) -> Result<(), String> {
    let remark = remark.map(str::trim).filter(|value| !value.is_empty());

    if included.is_none() && transfer_tax.is_none() && remark.is_none() {
        tx.execute(
            "DELETE FROM welfare_transfer_overrides
             WHERE detail_id = ?1 AND period_from = ?2 AND period_to = ?3",
            params![detail_id, period_from, period_to],
        )
        .map_err(|error| format!("清除人工结论失败：{error}"))?;
        return Ok(());
    }

    tx.execute(
        "INSERT INTO welfare_transfer_overrides
            (detail_id, period_from, period_to, included, transfer_tax, remark, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(detail_id, period_from, period_to) DO UPDATE SET
            included     = excluded.included,
            transfer_tax = excluded.transfer_tax,
            remark       = excluded.remark,
            updated_at   = excluded.updated_at",
        params![
            detail_id,
            period_from,
            period_to,
            included.map(|value| value as i64),
            transfer_tax,
            remark,
            updated_at
        ],
    )
    .map_err(|error| format!("保存人工结论失败：{error}"))?;

    Ok(())
}

/// 校验 detail_id 确实属于所选期间的认证清单，防止前端传入越界数据。
pub(crate) fn detail_in_period(
    tx: &Transaction<'_>,
    period_from: &str,
    period_to: &str,
    detail_id: i64,
) -> Result<bool, String> {
    tx.query_row(
        "SELECT EXISTS (
            SELECT 1 FROM invoice_details d
            JOIN certified_list cl ON cl.header_id = d.header_id
            WHERE d.id = ?1 AND cl.tax_period BETWEEN ?2 AND ?3
         )",
        params![detail_id, period_from, period_to],
        |row| row.get::<_, i64>(0),
    )
    .map(|exists| exists != 0)
    .map_err(|error| format!("校验发票明细归属失败：{error}"))
}