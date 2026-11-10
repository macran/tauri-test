// src-tauri/src/features/travel_expense_import/repository.rs
//
// 职责：
//   1. 检查 certified_list 是否已有数据
//   2. 根据 inv_unq_key 关联 invoice_headers.id
//   3. 更新 certified_list.voucher_no / posted_date / is_travel_expense
//   4. 返回 TravelExpenseImportSummary

use rusqlite::Connection;

use super::models::{ParsedTravelExpenseRow, TravelExpenseImportError, TravelExpenseImportSummary};

/// 批量更新 Travel Expense 凭证信息。
///
/// 在外部事务中调用：
///   - 调用方负责 BEGIN / COMMIT / ROLLBACK
///
/// 注意：
///   certified_list 表中没有 inv_unq_key，只有 header_id。
///   所以需要通过 invoice_headers.inv_unq_key 找到 invoice_headers.id，
///   再用 certified_list.header_id 更新。
pub fn update_travel_expense_vouchers(
    conn: &Connection,
    parsed_rows: &[ParsedTravelExpenseRow],
    mut errors: Vec<TravelExpenseImportError>,
) -> Result<TravelExpenseImportSummary, rusqlite::Error> {
    // ── Step 0：必须先导入认证清单 ───────────────────────────
    let certified_count: i64 =
        conn.query_row("SELECT COUNT(*) FROM certified_list", [], |row| row.get(0))?;

    if certified_count == 0 {
        return Ok(TravelExpenseImportSummary {
            updated: 0,
            error_count: errors.len() as u32,
            committed: false,
            rollback_reason: Some("请先导入认证清单，再导入 Travel Expense 凭证文件".to_string()),
            errors,
        });
    }

    let mut updated: u32 = 0;

    // ── Step 1：准备更新语句 ─────────────────────────────────
    //
    // 这里不直接 WHERE inv_unq_key，因为 certified_list 没有这个字段。
    //
    // affected == 0 的情况包括：
    //   1. invoice_headers 中不存在这个 inv_unq_key
    //   2. invoice_headers 中存在，但 certified_list 中没有对应 header_id
    //
    // 对前端统一表现为：认证清单中未找到对应发票。
    let mut stmt_update = conn.prepare_cached(
        "
        UPDATE certified_list
        SET voucher_no = ?1,
            posted_date = ?2,
            is_travel_expense = 1
        WHERE header_id = (
            SELECT id
            FROM invoice_headers
            WHERE inv_unq_key = ?3
            LIMIT 1
        )
        ",
    )?;

    // ── Step 2：逐行更新 ─────────────────────────────────────
    for row in parsed_rows {
        let affected = stmt_update.execute(rusqlite::params![
            row.voucher_no,
            row.posted_date,
            row.inv_unq_key,
        ])?;

        if affected == 1 {
            updated += 1;
        } else {
            errors.push(TravelExpenseImportError {
                row: row.excel_row,
                inv_unq_key: row.inv_unq_key.clone(),
                voucher_no: row.voucher_no.clone(),
                message: "认证清单中未找到对应 inv_unq_key".to_string(),
            });
        }
    }

    Ok(TravelExpenseImportSummary {
        updated,
        error_count: errors.len() as u32,
        committed: true, // 调用方会在 COMMIT 成功后确认
        rollback_reason: None,
        errors,
    })
}
