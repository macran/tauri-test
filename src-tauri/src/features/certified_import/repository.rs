// src-tauri/src/features/certified_import/repository.rs
//
// 职责：
//   1. 用 inv_unq_key 查找 invoice_headers.id
//   2. 组装 CertifiedList 并插入 certified_list 表
//   3. 返回 CertifiedImportSummary

use rusqlite::{Connection, OptionalExtension};

use crate::error::RowParseError;

use super::models::{CertifiedImportSummary, CertifiedList, ParsedCertifiedRow};

/// 批量写入认证清单。
/// 在外部事务中调用（调用方负责 BEGIN / COMMIT / ROLLBACK）。
pub fn insert_certified_list(
    conn: &Connection,
    parsed_rows: &[ParsedCertifiedRow],
    tax_period: &str,
    parse_errors: Vec<RowParseError>,
) -> Result<CertifiedImportSummary, rusqlite::Error> {
    let mut inserted: u32 = 0;
    let mut duplicate_count: u32 = 0;
    let mut header_not_found_count: u32 = 0;
    let mut header_not_found_keys: Vec<String> = Vec::new();

    let mut stmt_query =
        conn.prepare_cached("SELECT id FROM invoice_headers WHERE inv_unq_key = ?1 LIMIT 1")?;

    let mut stmt_insert = conn.prepare_cached(
        "INSERT OR IGNORE INTO certified_list (
            header_id, domestic_proof_no, valid_deduct_amount,
            invoice_tag, check_time, risk_status,
            tax_period, is_travel_expense, voucher_no, posted_date
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, NULL, NULL)",
    )?;

    for row in parsed_rows {
        // ── Step 1：查找 header_id ────────────────────────────
        let header_id: Option<i64> = stmt_query
            .query_row([&row.inv_unq_key], |r| r.get(0))
            .optional()?;

        let header_id = match header_id {
            Some(id) => id,
            None => {
                header_not_found_count += 1;
                header_not_found_keys.push(row.inv_unq_key.clone());
                continue;
            }
        };

        // ── Step 2：组装 CertifiedList ────────────────────────
        let record = CertifiedList {
            header_id,
            domestic_proof_no: row.domestic_proof_no.clone(),
            valid_deduct_amount: row.valid_deduct_amount,
            invoice_tag: row.invoice_tag.clone(),
            check_time: row.check_time.clone(),
            risk_status: row.risk_status,
            tax_period: tax_period.to_string(),
            is_travel_expense: 0,
            voucher_no: None,
            posted_date: None,
        };

        // ── Step 3：插入（IGNORE 处理重复）───────────────────
        let affected = stmt_insert.execute(rusqlite::params![
            record.header_id,
            record.domestic_proof_no,
            record.valid_deduct_amount,
            record.invoice_tag,
            record.check_time,
            record.risk_status,
            record.tax_period,
        ])?;

        if affected == 1 {
            inserted += 1;
        } else {
            duplicate_count += 1;
        }
    }

    let parse_error_count = parse_errors.len() as u32;

    Ok(CertifiedImportSummary {
        tax_period: tax_period.to_string(),
        inserted,
        parse_error_count,
        header_not_found_count,
        duplicate_count,
        committed: true, // 调用方设置最终值
        rollback_reason: None,
        parse_errors,
        header_not_found_keys,
    })
}
