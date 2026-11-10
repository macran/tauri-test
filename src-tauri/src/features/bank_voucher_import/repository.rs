// src-tauri/src/features/bank_voucher_import/repository.rs
//
// 职责：
//   1. 根据 Bank Excel 中的发票号码匹配 invoice_headers.id
//   2. 20 位数电票号码：查 e_num / inv_unq_key
//   3. 8 位传统发票号码：查 num
//   4. 校验 Excel 税额是否等于 invoice_headers.tax
//   5. 写入 bank_invoice_vouchers
//   6. 返回导入统计和错误详情

use rusqlite::{params, Connection};

use super::models::{BankVoucherErrorKind, BankVoucherImportError, BankVoucherImportSummary, ParsedBankVoucherRow};

#[derive(Debug, Clone)]
struct InvoiceHeaderMatch {
    id: i64,
    tax: i64,
    inv_unq_key: String,
}

pub fn insert_bank_vouchers(
    conn: &Connection,
    parsed_rows: &[ParsedBankVoucherRow],
    received_date: &str,
    prepared_by: &str,
    mut errors: Vec<BankVoucherImportError>,
) -> Result<BankVoucherImportSummary, rusqlite::Error> {
    let mut inserted: u32 = 0;
    let mut not_found_count: u32 = 0;
    let mut tax_mismatch_count: u32 = 0;
    let mut ambiguous_count: u32 = 0;

    let parse_error_count = errors
        .iter()
        .filter(|e| matches!(e.kind, BankVoucherErrorKind::Parse))
        .count() as u32;

    let mut stmt_insert = conn.prepare_cached(
        "
        INSERT INTO bank_invoice_vouchers (
            header_id,
            voucher_no,
            posted_date,
            received_date,
            prepared_by

        )
        VALUES (?1, ?2, ?3, ?4, ?5)
        ON CONFLICT(header_id) DO UPDATE SET
            voucher_no    = excluded.voucher_no,
            posted_date   = excluded.posted_date,
            received_date = excluded.received_date,
            prepared_by   = excluded.prepared_by

        ",
    )?;

    for row in parsed_rows {
        let header = match find_invoice_header(conn, &row.invoice_no)? {
            FindHeaderResult::Found(header) => header,
            FindHeaderResult::NotFound => {
                not_found_count += 1;

                errors.push(BankVoucherImportError {
                    row: row.excel_row,
                    invoice_no: row.invoice_no.clone(),
                    voucher_no: row.voucher_no.clone(),
                    kind: BankVoucherErrorKind::NotFound,
                    message: format!(
                        "未在 invoice_headers 中找到对应发票。发票号码：{}",
                        row.invoice_no
                    ),
                });

                continue;
            }
            FindHeaderResult::Ambiguous(count) => {
                ambiguous_count += 1;

                errors.push(BankVoucherImportError {
                    row: row.excel_row,
                    invoice_no: row.invoice_no.clone(),
                    voucher_no: row.voucher_no.clone(),
                    kind: BankVoucherErrorKind::Ambiguous,
                    message: format!(
                        "发票号码匹配到 {} 张发票，无法唯一确定 header_id。发票号码：{}",
                        count, row.invoice_no
                    ),
                });

                continue;
            }
        };

        if header.tax != row.tax_fen {
            tax_mismatch_count += 1;

            errors.push(BankVoucherImportError {
                row: row.excel_row,
                invoice_no: row.invoice_no.clone(),
                voucher_no: row.voucher_no.clone(),
                kind: BankVoucherErrorKind::TaxMismatch,
                message: format!(
                    "税额不一致。Excel 税额：{} 分；数据库税额：{} 分；inv_unq_key：{}",
                    row.tax_fen, header.tax, header.inv_unq_key
                ),
            });

            continue;
        }

        stmt_insert.execute(params![
            header.id,
            row.voucher_no,
            row.posted_date,
            received_date,
            prepared_by,
        ])?;

        inserted += 1;
    }

    let error_count = errors.len() as u32;

    Ok(BankVoucherImportSummary {
        inserted,
        not_found_count,
        tax_mismatch_count,
        ambiguous_count,
        parse_error_count,
        error_count,
        committed: true,
        rollback_reason: None,
        errors,
    })
}

enum FindHeaderResult {
    Found(InvoiceHeaderMatch),
    NotFound,
    Ambiguous(usize),
}

fn find_invoice_header(
    conn: &Connection,
    invoice_no: &str,
) -> Result<FindHeaderResult, rusqlite::Error> {
    match invoice_no.len() {
        // 数电票号码：20 位
        20 => find_by_e_num(conn, invoice_no),

        // 老旧发票号码：8 位
        8 => find_by_num(conn, invoice_no),

        // 理论上 parser 已经拦截，这里兜底
        _ => Ok(FindHeaderResult::NotFound),
    }
}

fn find_by_e_num(conn: &Connection, e_num: &str) -> Result<FindHeaderResult, rusqlite::Error> {
    let mut stmt = conn.prepare_cached(
        "
        SELECT id, tax, inv_unq_key
        FROM invoice_headers
        WHERE TRIM(e_num) = ?1
           OR inv_unq_key = ?1
        LIMIT 2
        ",
    )?;

    let matches = stmt
        .query_map([e_num], |row| {
            Ok(InvoiceHeaderMatch {
                id: row.get(0)?,
                tax: row.get(1)?,
                inv_unq_key: row.get(2)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    match matches.len() {
        0 => Ok(FindHeaderResult::NotFound),
        1 => Ok(FindHeaderResult::Found(matches[0].clone())),
        n => Ok(FindHeaderResult::Ambiguous(n)),
    }
}

fn find_by_num(conn: &Connection, num: &str) -> Result<FindHeaderResult, rusqlite::Error> {
    let mut stmt = conn.prepare_cached(
        "
        SELECT id, tax, inv_unq_key
        FROM invoice_headers
        WHERE TRIM(num) = ?1
        LIMIT 2
        ",
    )?;

    let matches = stmt
        .query_map([num], |row| {
            Ok(InvoiceHeaderMatch {
                id: row.get(0)?,
                tax: row.get(1)?,
                inv_unq_key: row.get(2)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    match matches.len() {
        0 => Ok(FindHeaderResult::NotFound),
        1 => Ok(FindHeaderResult::Found(matches[0].clone())),
        n => Ok(FindHeaderResult::Ambiguous(n)),
    }
}
