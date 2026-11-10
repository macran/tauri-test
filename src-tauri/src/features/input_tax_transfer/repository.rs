use std::collections::{HashMap, HashSet};

use rusqlite::{params, params_from_iter, OptionalExtension, Transaction};

use super::models::{MatchCandidate, RawReviewRow, RuleForMatch};
use crate::features::period_close::repository::{read_confirm, CONFIRM_COLS};

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

/// 取出认证期间内「有凭证号」的发票明细。无凭证号的一律不参与匹配。
/// 已在任何纳税所属期**确认**（含已锁定）转出的明细不再参与；其余明细——包括别的所属期里
/// 机器命中但没确认的——都可以参与。
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
               AND TRIM(COALESCE(cl.voucher_no, '')) <> ''
               AND NOT EXISTS (SELECT 1 FROM welfare_transfer_confirmed_lines c
                                WHERE c.detail_id = d.id)",
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

/// 删除本纳税所属期里本次不再命中的系统结果；rule_hits 由 ON DELETE CASCADE 连带清除。
/// 仍然命中的行保留（id 不变），已确认的明细（含补充批次里批次 1 的）不动，不触碰 welfare_transfer_overrides。
pub(crate) fn delete_stale_results(
    tx: &Transaction<'_>,
    run_id: i64,
    keep_detail_ids: &HashSet<i64>,
) -> Result<(), String> {
    let existing: Vec<(i64, i64)> = {
        let mut statement = tx
            .prepare(
                "SELECT r.id, r.detail_id FROM welfare_transfer_results r
                 WHERE r.run_id = ?1
                   AND NOT EXISTS (SELECT 1 FROM welfare_transfer_confirmed_lines c
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
            tx.execute(
                "DELETE FROM welfare_transfer_results WHERE id = ?1",
                params![id],
            )
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
            "SELECT id FROM welfare_transfer_results WHERE run_id = ?1 AND detail_id = ?2",
            params![run_id, candidate.detail_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| format!("读取已有匹配结果失败：{error}"))?;
    if let Some(id) = existing {
        let mut old: Vec<i64> = tx
            .prepare("SELECT rule_id FROM welfare_transfer_rule_hits WHERE result_id = ?1")
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
        "INSERT INTO welfare_transfer_results (run_id, detail_id, header_id, matched_at)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(run_id, detail_id) DO UPDATE SET matched_at = excluded.matched_at",
        params![run_id, candidate.detail_id, candidate.header_id, matched_at],
    )
    .map_err(|error| format!("保存匹配结果失败：{error}"))?;

    let result_id: i64 = tx
        .query_row(
            "SELECT id FROM welfare_transfer_results WHERE run_id = ?1 AND detail_id = ?2",
            params![run_id, candidate.detail_id],
            |row| row.get(0),
        )
        .map_err(|error| format!("读取匹配结果 ID 失败：{error}"))?;

    tx.execute(
        "DELETE FROM welfare_transfer_rule_hits WHERE result_id = ?1",
        params![result_id],
    )
    .map_err(|error| format!("重置命中规则失败：{error}"))?;

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

/// 统计该纳税所属期已有的人工结论条数，向用户说明有多少条不受重算影响。
pub(crate) fn count_overrides(tx: &Transaction<'_>, run_id: i64) -> Result<usize, String> {
    tx.query_row(
        "SELECT COUNT(*) FROM welfare_transfer_overrides WHERE run_id = ?1",
        params![run_id],
        |row| row.get::<_, i64>(0),
    )
    .map(|count| count as usize)
    .map_err(|error| format!("统计人工结论失败：{error}"))
}

// ───────────────── 核对列表主干查询 ─────────────────

const REVIEW_SELECT: &str = "SELECT
        d.id, d.header_id, d.line_no,
        cl.tax_period,
        CASE WHEN TRIM(COALESCE(cl.voucher_no, '')) = ''
             THEN NULL ELSE cl.voucher_no END,
        h.inv_unq_key, h.issue_date,
        s.tax_no, s.name,
        d.goods_name, d.tax_code, d.goods_category, d.tax_rate,
        d.amount, d.tax, d.total,
        r.id, o.included, o.transfer_tax, o.remark,
        cf.transfer_tax AS cf_tax, cf.decision AS cf_decision";

/// 行集 = 本纳税所属期来源范围（认证期间）内的发票明细 ∪ 本所属期已有结论的明细。
/// 后者保证缩小或改了范围后，已入库的行也不会消失。
/// 已确认（本所属期或别的所属期）的明细都显示，别的所属期确认的在前端标记为锁定。
pub(crate) fn fetch_review_rows(
    tx: &Transaction<'_>,
    run_id: i64,
    period_from: &str,
    period_to: &str,
    confirmed_only: bool,
) -> Result<Vec<RawReviewRow>, String> {
    // 查询页「仅已确认」只看已确认快照；复核页要看全部候选。
    let scope = if confirmed_only {
        "cf.id IS NOT NULL AND cf.run_id = ?3"
    } else {
        "(cl.tax_period BETWEEN ?1 AND ?2 OR r.id IS NOT NULL OR o.id IS NOT NULL)"
    };
    let sql = format!(
        "{REVIEW_SELECT}, {CONFIRM_COLS}
         FROM certified_list cl
         JOIN invoice_headers h ON h.id = cl.header_id
         JOIN sellers s         ON s.id = h.seller_id
         JOIN invoice_details d ON d.header_id = cl.header_id
         LEFT JOIN welfare_transfer_results r
                ON r.detail_id = d.id AND r.run_id = ?3
         LEFT JOIN welfare_transfer_overrides o
                ON o.detail_id = d.id AND o.run_id = ?3
         LEFT JOIN welfare_transfer_confirmed_lines cf ON cf.detail_id = d.id
         LEFT JOIN tax_run_batches b ON b.id = cf.batch_id
         LEFT JOIN tax_runs tr       ON tr.id = cf.run_id
         WHERE {scope}
         ORDER BY cl.tax_period, d.header_id, d.line_no"
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
                cert_period: row.get(3)?,
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
                confirmed_tax: row.get("cf_tax")?,
                confirmed_decision: row.get("cf_decision")?,
                confirm: read_confirm(row)?,
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
    run_id: i64,
    detail_id: i64,
    included: Option<bool>,
    transfer_tax: Option<i64>,
    remark: Option<&str>,
    updated_at: &str,
) -> Result<(), String> {
    let remark = remark.map(str::trim).filter(|value| !value.is_empty());

    if included.is_none() && transfer_tax.is_none() && remark.is_none() {
        tx.execute(
            "DELETE FROM welfare_transfer_overrides WHERE detail_id = ?1 AND run_id = ?2",
            params![detail_id, run_id],
        )
        .map_err(|error| format!("清除人工结论失败：{error}"))?;
        return Ok(());
    }

    tx.execute(
        "INSERT INTO welfare_transfer_overrides
            (run_id, detail_id, included, transfer_tax, remark, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(run_id, detail_id) DO UPDATE SET
            included     = excluded.included,
            transfer_tax = excluded.transfer_tax,
            remark       = excluded.remark,
            updated_at   = excluded.updated_at",
        params![
            run_id,
            detail_id,
            included.map(|value| value as i64),
            transfer_tax,
            remark,
            updated_at
        ],
    )
    .map_err(|error| format!("保存人工结论失败：{error}"))?;

    drop_if_same_as_system(tx, run_id, detail_id)
}

/// 人工结论与系统结论完全一致（转出与否相同、税额等于系统税额、没有备注）时，按「没操作过」处理：删掉这一行。
fn drop_if_same_as_system(
    tx: &Transaction<'_>,
    run_id: i64,
    detail_id: i64,
) -> Result<(), String> {
    tx.execute(
        "DELETE FROM welfare_transfer_overrides
         WHERE detail_id = ?1 AND run_id = ?2 AND remark IS NULL
           AND (included IS NULL OR included = EXISTS (
                SELECT 1 FROM welfare_transfer_results r WHERE r.detail_id = ?1 AND r.run_id = ?2))
           AND (transfer_tax IS NULL OR transfer_tax = CASE
                WHEN EXISTS (SELECT 1 FROM welfare_transfer_results r WHERE r.detail_id = ?1 AND r.run_id = ?2)
                THEN (SELECT tax FROM invoice_details WHERE id = ?1) ELSE 0 END)",
        params![detail_id, run_id],
    )
    .map_err(|error| format!("清理与系统一致的人工结论失败：{error}"))?;
    Ok(())
}

/// 校验 detail_id 能在本纳税所属期修改：属于所选认证期间，或本所属期已有结论；
/// 且没有在任何所属期被确认（已确认/已锁定的明细不可改）。
pub(crate) fn detail_editable(
    tx: &Transaction<'_>,
    run_id: i64,
    period_from: &str,
    period_to: &str,
    detail_id: i64,
) -> Result<bool, String> {
    tx.query_row(
        "SELECT EXISTS (
            SELECT 1 FROM invoice_details d
            LEFT JOIN certified_list cl ON cl.header_id = d.header_id
            WHERE d.id = ?1
              AND (cl.tax_period BETWEEN ?3 AND ?4
                   OR EXISTS (SELECT 1 FROM welfare_transfer_results r
                               WHERE r.detail_id = d.id AND r.run_id = ?2)
                   OR EXISTS (SELECT 1 FROM welfare_transfer_overrides o
                               WHERE o.detail_id = d.id AND o.run_id = ?2))
              AND NOT EXISTS (SELECT 1 FROM welfare_transfer_confirmed_lines c
                               WHERE c.detail_id = d.id)
         )",
        params![detail_id, run_id, period_from, period_to],
        |row| row.get::<_, i64>(0),
    )
    .map(|exists| exists != 0)
    .map_err(|error| format!("校验发票明细归属失败：{error}"))
}

// ───────────────── 确认快照 ─────────────────

pub(crate) struct ConfirmLine {
    pub detail_id: i64,
    pub header_id: i64,
    pub cert_period: String,
    pub detail_tax: i64,
    pub transfer_tax: i64,
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
        "INSERT INTO welfare_transfer_confirmed_lines
            (run_id, batch_id, detail_id, header_id, cert_period, detail_tax, transfer_tax,
             decision, remark, confirmed_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            run_id,
            batch_id,
            line.detail_id,
            line.header_id,
            line.cert_period,
            line.detail_tax,
            line.transfer_tax,
            line.decision,
            line.remark,
            confirmed_at
        ],
    )
    .map_err(|error| format!("保存已确认明细失败：{error}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    /// 明细 1、2 都在 202609 认证、有凭证号；明细 3 在 202610 认证。
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
               VALUES (1, 'E1', 1, '2026-09-05', 100, 9, 109, 1, 1, 1, 1, 1),
                      (2, 'E2', 1, '2026-09-06', 100, 9, 109, 1, 1, 1, 1, 1),
                      (3, 'E3', 1, '2026-10-06', 100, 9, 109, 1, 1, 1, 1, 1);
             INSERT INTO invoice_details(id, header_id, line_no, goods_name, amount, tax_rate, tax, total)
               VALUES (1, 1, 1, '甲', 100, '9%', 9, 109), (2, 2, 1, '乙', 100, '9%', 9, 109), (3, 3, 1, '丙', 100, '9%', 9, 109);
             INSERT INTO certified_list(header_id, valid_deduct_amount, invoice_tag, check_time, risk_status, tax_period, voucher_no)
               VALUES (1, 9, 't', 'now', 0, '202609', 'V1'), (2, 9, 't', 'now', 0, '202609', 'V2'), (3, 9, 't', 'now', 0, '202610', 'V3');
             INSERT INTO tax_runs(id, module, filing_from, filing_to, src_from, src_to, created_by, created_at)
               VALUES (1, 'input_tax_transfer', '202609', '202609', '202609', '202610', 'u', 't'),
                      (2, 'input_tax_transfer', '202610', '202610', '202609', '202610', 'u', 't');
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
                cert_period: "202609".into(),
                detail_tax: 9,
                transfer_tax: 9,
                decision: "auto",
                remark: None,
            },
            "now",
        )
        .unwrap();
    }

    fn candidate_ids(tx: &Transaction<'_>) -> Vec<i64> {
        let mut v: Vec<i64> = fetch_match_candidates(tx, "202609", "202610")
            .unwrap()
            .iter()
            .map(|c| c.detail_id)
            .collect();
        v.sort();
        v
    }

    #[test]
    fn only_confirmed_details_are_excluded_from_other_runs() {
        let mut conn = seeded();
        let tx = conn.transaction().unwrap();

        // 202609 里明细 1 机器命中（未确认）、明细 2 被人工设为转出（未确认）：别的所属期仍可匹配它们
        upsert_result(&tx, &candidate(1), 1, "now", &[]).unwrap();
        upsert_override(&tx, 1, 2, Some(true), None, None, "now").unwrap();
        assert_eq!(candidate_ids(&tx), vec![1, 2, 3]);

        // 明细 1 在 202609 确认后，别的所属期不再把它当候选；未确认的 2、3 仍是候选
        confirm(&tx, 1, 1, 1);
        assert_eq!(candidate_ids(&tx), vec![2, 3]);

        // 取消确认（删快照）后，明细 1 又能参与
        tx.execute("DELETE FROM welfare_transfer_confirmed_lines", []).unwrap();
        assert_eq!(candidate_ids(&tx), vec![1, 2, 3]);
    }

    #[test]
    fn same_detail_can_have_results_in_two_runs() {
        let mut conn = seeded();
        let tx = conn.transaction().unwrap();
        upsert_result(&tx, &candidate(1), 1, "t1", &[]).unwrap();
        upsert_result(&tx, &candidate(1), 2, "t1", &[]).unwrap();
        let n: i64 = tx
            .query_row("SELECT COUNT(*) FROM welfare_transfer_results WHERE detail_id = 1", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 2);
        // 人工结论按 (run, detail) 各自保存
        upsert_override(&tx, 1, 1, Some(false), None, None, "t").unwrap();
        upsert_override(&tx, 2, 1, Some(true), Some(5), None, "t").unwrap();
        assert_eq!(count_overrides(&tx, 1).unwrap(), 1);
        assert_eq!(count_overrides(&tx, 2).unwrap(), 1);
    }

    #[test]
    fn rematch_keeps_overrides_and_confirmed_and_only_removes_stale_results() {
        let mut conn = seeded();
        let tx = conn.transaction().unwrap();

        upsert_result(&tx, &candidate(1), 1, "t1", &[]).unwrap();
        upsert_result(&tx, &candidate(2), 1, "t1", &[]).unwrap();
        upsert_override(&tx, 1, 2, Some(false), None, None, "t1").unwrap(); // 明细 2 此时命中，不转出是真修改
        let id_of_1: i64 = tx
            .query_row("SELECT id FROM welfare_transfer_results WHERE detail_id = 1", [], |r| r.get(0))
            .unwrap();

        // 重新匹配：只有明细 1 仍然命中
        upsert_result(&tx, &candidate(1), 1, "t2", &[]).unwrap();
        delete_stale_results(&tx, 1, &HashSet::from([1])).unwrap();

        let id_after: i64 = tx
            .query_row("SELECT id FROM welfare_transfer_results WHERE detail_id = 1", [], |r| r.get(0))
            .unwrap();
        assert_eq!(id_after, id_of_1);
        let left: i64 = tx
            .query_row("SELECT COUNT(*) FROM welfare_transfer_results", [], |r| r.get(0))
            .unwrap();
        assert_eq!(left, 1);
        assert_eq!(count_overrides(&tx, 1).unwrap(), 1);

        // 三项全空 = 恢复系统结论，删除人工行
        upsert_override(&tx, 1, 2, None, None, None, "t3").unwrap();
        assert_eq!(count_overrides(&tx, 1).unwrap(), 0);

        // 已确认的明细不会被当作过期结果删掉（补充批次重新匹配的场景）
        confirm(&tx, 1, 1, 1);
        delete_stale_results(&tx, 1, &HashSet::new()).unwrap();
        let kept: i64 = tx
            .query_row("SELECT COUNT(*) FROM welfare_transfer_results WHERE detail_id = 1", [], |r| r.get(0))
            .unwrap();
        assert_eq!(kept, 1);
    }

    #[test]
    fn override_equal_to_system_is_dropped() {
        let mut conn = seeded();
        let tx = conn.transaction().unwrap();
        upsert_result(&tx, &candidate(1), 1, "t", &[]).unwrap(); // 明细 1 系统命中（转出，税额 9）
        upsert_override(&tx, 1, 1, Some(true), None, None, "t").unwrap();
        assert_eq!(count_overrides(&tx, 1).unwrap(), 0);
        upsert_override(&tx, 1, 1, Some(true), Some(9), None, "t").unwrap();
        assert_eq!(count_overrides(&tx, 1).unwrap(), 0);
        upsert_override(&tx, 1, 1, Some(false), None, None, "t").unwrap();
        assert_eq!(count_overrides(&tx, 1).unwrap(), 1);
        upsert_override(&tx, 1, 1, Some(true), Some(5), None, "t").unwrap();
        assert_eq!(count_overrides(&tx, 1).unwrap(), 1);
        upsert_override(&tx, 1, 1, Some(true), None, Some("备注"), "t").unwrap();
        assert_eq!(count_overrides(&tx, 1).unwrap(), 1);
        upsert_override(&tx, 1, 2, Some(false), None, None, "t").unwrap();
        assert_eq!(count_overrides(&tx, 1).unwrap(), 1);
    }

    #[test]
    fn rematch_with_same_result_keeps_matched_at() {
        let mut conn = seeded();
        let tx = conn.transaction().unwrap();
        let at = |tx: &Transaction<'_>| -> String {
            tx.query_row("SELECT matched_at FROM welfare_transfer_results WHERE detail_id = 1", [], |r| r.get(0)).unwrap()
        };
        upsert_result(&tx, &candidate(1), 1, "t1", &[]).unwrap();
        upsert_result(&tx, &candidate(1), 1, "t2", &[]).unwrap(); // 没变：不刷新
        assert_eq!(at(&tx), "t1");
        tx.execute("INSERT INTO tax_match_rules(id, scope, rule_category, tax_name_match_type, tax_name_values, source, created_at, rule_signature) VALUES (900, 'input_tax_transfer', 'x', 'eq', 'x', 'manual', 'now', randomblob(32))", []).unwrap();
        upsert_result(&tx, &candidate(1), 1, "t3", &[900]).unwrap(); // 命中集合变了：刷新
        assert_eq!(at(&tx), "t3");
        upsert_result(&tx, &candidate(1), 1, "t4", &[900]).unwrap();
        assert_eq!(at(&tx), "t3");
    }

    #[test]
    fn editable_only_when_in_range_or_stored_and_not_confirmed() {
        let mut conn = seeded();
        let tx = conn.transaction().unwrap();
        // 明细 3 在 202610 认证：run 1 范围 202609 内不可改，run 2 范围内可改
        assert!(!detail_editable(&tx, 1, "202609", "202609", 3).unwrap());
        assert!(detail_editable(&tx, 2, "202610", "202610", 3).unwrap());
        // 已有人工结论的明细即使范围缩小也仍可改
        upsert_override(&tx, 1, 3, Some(true), None, None, "t").unwrap();
        assert!(detail_editable(&tx, 1, "202609", "202609", 3).unwrap());
        // 任何所属期确认后，所有 run 都不可改
        confirm(&tx, 2, 2, 3);
        assert!(!detail_editable(&tx, 1, "202609", "202609", 3).unwrap());
        assert!(!detail_editable(&tx, 2, "202610", "202610", 3).unwrap());
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
        // 在 run 2 的核对表里，明细 1（认证期间 202609）显示为别的所属期已确认，并带凭证信息
        let rows = fetch_review_rows(&tx, 2, "202609", "202610", false).unwrap();
        let r1 = rows.iter().find(|r| r.detail_id == 1).unwrap();
        let c = r1.confirm.as_ref().unwrap();
        assert_eq!((c.run_id, c.batch_no, c.voucher_no.as_deref()), (1, 1, Some("JV01")));
        assert_eq!(r1.confirmed_tax, Some(9));
        // 「仅已确认」只列本 run 的快照，不含别的 run 的
        assert!(fetch_review_rows(&tx, 2, "202609", "202610", true).unwrap().is_empty());
        assert_eq!(fetch_review_rows(&tx, 1, "202609", "202610", true).unwrap().len(), 1);
    }
}
