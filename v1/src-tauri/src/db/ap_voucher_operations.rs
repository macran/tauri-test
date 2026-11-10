// src-tauri/src/db/ap_voucher_operations.rs

use rusqlite::{Connection, OptionalExtension};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::models::ap_voucher::ApVoucherRow;
use crate::models::dto::{ApVoucherImportError, ApVoucherImportSummary};
use crate::parser::ap_voucher::ParsedApVoucherRow;

pub fn import_ap_vouchers(
    conn: &Connection,
    parsed_rows: Vec<ParsedApVoucherRow>,
    received_date: &str,
    preparer: &str,
    parse_errors: Vec<ApVoucherImportError>,
) -> Result<ApVoucherImportSummary, rusqlite::Error> {
    let mut errors = parse_errors;
    let mut rows_to_insert: Vec<ApVoucherRow> = Vec::new();
    let mut vendor_new_count: u32 = 0;
    let mut vendor_existing_count: u32 = 0;

    for parsed in &parsed_rows {
        // ── 1a. upsert vendor ────────────────────────────────
        let (vendor_id, is_new) = upsert_vendor(conn, &parsed.vendor_code, &parsed.vendor_name)?;
        if is_new {
            vendor_new_count += 1;
        } else {
            vendor_existing_count += 1;
        }

        // ── 1b. 查找 header_id ───────────────────────────────
        let header_id = match resolve_header_id(conn, parsed) {
            Ok(id) => id,
            Err(msg) => {
                errors.push(make_err(
                    parsed.excel_row,
                    parsed.barcode.clone(),
                    Some(parsed.inv_num.clone()),
                    "发票匹配失败",
                    &msg,
                ));
                continue;
            }
        };

        // ── 1c. 核对金额（只要找到 header_id 就核对）────────
        if let Some(hid) = header_id {
            if let Err(msg) = verify_amounts(conn, parsed, hid) {
                errors.push(make_err(
                    parsed.excel_row,
                    parsed.barcode.clone(),
                    Some(parsed.inv_num.clone()),
                    "金额核对失败",
                    &msg,
                ));
                continue;
            }

            // ── 1d. 增票额外核对 inv_type ────────────────────
            if parsed.inv_type.needs_inv_type_check() {
                if let Err(msg) = verify_inv_type(conn, hid) {
                    errors.push(make_err(
                        parsed.excel_row,
                        parsed.barcode.clone(),
                        Some(parsed.inv_num.clone()),
                        "发票类型核对失败",
                        &msg,
                    ));
                    continue;
                }
            }
        }

        let invoice_no_display = build_invoice_no_display(parsed, header_id);
        let content_fingerprint = build_content_fingerprint(parsed, header_id, vendor_id);

        rows_to_insert.push(ApVoucherRow {
            header_id,
            vendor_id,
            barcode: parsed.barcode.clone(),
            invoice_no_display,
            entry_no: parsed.entry_no.clone(),
            amount: parsed.amount,
            tax_rate: parsed.tax_rate.clone(),
            tax: parsed.tax,
            total: parsed.total,
            inv_type: parsed.inv_type,
            po_no: parsed.po_no.clone(),
            voucher_no: parsed.voucher_no.clone(),
            invoice_date: parsed.invoice_date.clone(),
            entry_date: parsed.entry_date.clone(),
            remark: parsed.remark.clone(),
            tax_certified: parsed.tax_certified,
            received_date: received_date.to_string(),
            preparer: preparer.to_string(),
            content_fingerprint,
        });
    }

    if !errors.is_empty() {
        // 如果收集到了任何错误（无论是解析错误还是业务校验错误）
        // 直接返回！不执行下面的插入逻辑！
        // 注意这里返回的是 Ok，但 committed 标记为 false
        return Ok(ApVoucherImportSummary {
            inserted: 0,
            updated: 0,
            skipped: 0,
            error_count: errors.len() as u32,
            vendor_new_count: 0, // 因为要回滚，所以这些计数对前端没意义，置零
            vendor_existing_count: 0,
            committed: false,
            rollback_reason: Some("存在校验错误，已阻止入库".to_string()),
            errors, // 把所有错误明细带给前端
        });
    }
    // ── Step 2：按 content_fingerprint 做 UPSERT ──────────────
    // 新身份 → 插入；身份相同但凭证号不同（红冲重做）→ 覆盖凭证号等字段；
    // 身份和凭证号都相同（完全重复）→ WHERE 条件为假，DO UPDATE 不生效，视为忽略。
    let mut inserted: u32 = 0;
    let mut updated: u32 = 0;
    let mut skipped: u32 = 0;

    let mut exists_stmt = conn.prepare_cached(
        "SELECT 1 FROM ap_invoice_vouchers WHERE content_fingerprint = ?1",
    )?;
    let mut upsert_stmt = conn.prepare_cached(
        "INSERT INTO ap_invoice_vouchers (
            header_id, vendor_id, barcode, invoice_no_display, entry_no,
            amount, tax_rate, tax, total,
            inv_type, po_no, voucher_no, invoice_date, entry_date,
            remark, tax_certified, received_date, preparer, content_fingerprint
        ) VALUES (
            ?1,  ?2,  ?3,  ?4,  ?5,
            ?6,  ?7,  ?8,  ?9,
            ?10, ?11, ?12, ?13, ?14,
            ?15, ?16, ?17, ?18, ?19
        )
        ON CONFLICT(content_fingerprint) DO UPDATE SET
            voucher_no    = excluded.voucher_no,
            entry_no      = excluded.entry_no,
            invoice_date  = excluded.invoice_date,
            entry_date    = excluded.entry_date,
            remark        = excluded.remark,
            tax_certified = excluded.tax_certified,
            received_date = excluded.received_date,
            preparer      = excluded.preparer
        WHERE ap_invoice_vouchers.voucher_no <> excluded.voucher_no",
    )?;

    for row in &rows_to_insert {
        let already_exists = exists_stmt
            .query_row(rusqlite::params![row.content_fingerprint], |_| Ok(()))
            .optional()?
            .is_some();

        upsert_stmt.execute(rusqlite::params![
            row.header_id,
            row.vendor_id,
            row.barcode,
            row.invoice_no_display,
            row.entry_no,
            row.amount,
            row.tax_rate,
            row.tax,
            row.total,
            row.inv_type as i32,
            row.po_no,
            row.voucher_no,
            row.invoice_date,
            row.entry_date,
            row.remark,
            row.tax_certified as i32,
            row.received_date,
            row.preparer,
            row.content_fingerprint,
        ])?;

        if !already_exists {
            inserted += 1;
        } else if conn.changes() > 0 {
            updated += 1;
        } else {
            skipped += 1;
        }
    }

    Ok(ApVoucherImportSummary {
        inserted,
        updated,
        skipped,
        error_count: errors.len() as u32,
        vendor_new_count,
        vendor_existing_count,
        committed: true,
        rollback_reason: None,
        errors,
    })
}

// ─────────────────────────────────────────────────────────────
// 原始发票号展示字段：
//   - 能匹配到 header_id（增票/普票/部分红票）→ 旧式发票用「代码-号码」，
//     数电票直接用号码
//   - 匹配不到 header_id（收据、通知单、票据等）→ Excel 的发票号码列
//     可能是占位文本（如「通知书」），不具区分度，改用 barcode+凭证号构造
// ─────────────────────────────────────────────────────────────
fn build_invoice_no_display(parsed: &ParsedApVoucherRow, header_id: Option<i64>) -> String {
    if header_id.is_some() {
        match parsed.inv_code.as_deref().filter(|c| !c.trim().is_empty()) {
            Some(code) => format!("{}-{}", code, parsed.inv_num),
            None => parsed.inv_num.clone(),
        }
    } else {
        format!(
            "{}-{}",
            parsed.barcode.as_deref().unwrap_or(""),
            parsed.voucher_no
        )
    }
}

// ─────────────────────────────────────────────────────────────
// 业务身份指纹：
//   header_id 存在时以 header_id 为锚点，否则退化为 barcode。
//   再加上 vendor_id / amount / tax / total / tax_rate / inv_type / po_no /
//   invoice_date 一起做 sha256。
//   故意不含 voucher_no、entry_no、entry_date、remark、tax_certified、
//   received_date、preparer —— 这些字段允许在导入时被新数据覆盖，
//   而不触发"判定为新业务记录"。
// ─────────────────────────────────────────────────────────────
#[derive(Serialize)]
struct FingerprintPayload<'a> {
    anchor: String,
    vendor_id: i64,
    amount: i64,
    tax: i64,
    total: i64,
    tax_rate: Option<&'a str>,
    inv_type: u8,
    po_no: Option<&'a str>,
    invoice_date: Option<&'a str>,
}

pub(crate) fn build_content_fingerprint(
    parsed: &ParsedApVoucherRow,
    header_id: Option<i64>,
    vendor_id: i64,
) -> Vec<u8> {
    fingerprint(
        header_id,
        parsed.barcode.as_deref(),
        vendor_id,
        parsed.amount,
        parsed.tax,
        parsed.total,
        parsed.tax_rate.as_deref(),
        parsed.inv_type as u8,
        parsed.po_no.as_deref(),
        parsed.invoice_date.as_deref(),
    )
}

/// 指纹的原始字段版本：导入与"编辑凭证后重算指纹"共用，保证两处口径一致
#[allow(clippy::too_many_arguments)]
pub(crate) fn fingerprint(
    header_id: Option<i64>,
    barcode: Option<&str>,
    vendor_id: i64,
    amount: i64,
    tax: i64,
    total: i64,
    tax_rate: Option<&str>,
    inv_type: u8,
    po_no: Option<&str>,
    invoice_date: Option<&str>,
) -> Vec<u8> {
    let anchor = match header_id {
        Some(id) => format!("H:{}", id),
        None => format!("B:{}", barcode.unwrap_or("")),
    };

    let payload = FingerprintPayload {
        anchor,
        vendor_id,
        amount,
        tax,
        total,
        tax_rate,
        inv_type,
        po_no,
        invoice_date,
    };

    let canonical_bytes =
        serde_json::to_vec(&payload).expect("FingerprintPayload 序列化不应失败");
    Sha256::digest(&canonical_bytes).to_vec()
}

// ─────────────────────────────────────────────────────────────
// upsert vendor
// 用你现有的 ON CONFLICT RETURNING 模式：
//   - vendor_code 已存在 → 更新 name，返回已有 id，is_new = false
//   - vendor_code 不存在 → 插入，返回新 id，is_new = true
// ─────────────────────────────────────────────────────────────
pub(crate) fn upsert_vendor(
    conn: &Connection,
    vendor_code: &str,
    vendor_name: &str,
) -> Result<(i64, bool), rusqlite::Error> {
    // 先查是否已存在，用于判断 is_new
    let existing: Option<i64> = conn
        .query_row(
            "SELECT id FROM vendors WHERE vendor_code = ?1",
            rusqlite::params![vendor_code],
            |row| row.get(0),
        )
        .optional()?;

    let is_new = existing.is_none();

    // ON CONFLICT 更新 name（供应商改名场景），RETURNING id
    let id: i64 = conn.query_row(
        "INSERT INTO vendors (vendor_code, vendor_name)
         VALUES (?1, ?2)
         ON CONFLICT(vendor_code) DO UPDATE SET vendor_name = excluded.vendor_name
         RETURNING id",
        rusqlite::params![vendor_code, vendor_name],
        |row| row.get(0),
    )?;

    Ok((id, is_new))
}

// ─────────────────────────────────────────────────────────────
// 根据发票号码/代码查找 header_id
// ─────────────────────────────────────────────────────────────
fn resolve_header_id(
    conn: &Connection,
    parsed: &ParsedApVoucherRow,
) -> Result<Option<i64>, String> {
    let inv_num = parsed.inv_num.trim();
    let inv_code = parsed.inv_code.as_deref().unwrap_or("").trim();

    if inv_num.is_empty() {
        if parsed.inv_type.requires_header() {
            return Err(format!(
                "发票类型为「{}」但发票号码为空，无法匹配发票",
                parsed.inv_type
            ));
        }
        return Ok(None);
    }

    let id_opt: Option<i64> = if inv_num.len() == 20 {
        // 数电票：直接用发票号码查 inv_unq_key
        query_by_unq_key(conn, inv_num)?
    } else if !inv_code.is_empty() {
        // 老式发票 + 有发票代码
        let key = format!("{}-{}", inv_code, inv_num);
        query_by_unq_key(conn, &key)?
    } else {
        // 老式发票 + 无发票代码：用 num 字段查，多条时核对含税金额
        query_by_num(conn, inv_num, parsed.total)?
    };

    match id_opt {
        Some(id) => Ok(Some(id)),
        None => {
            if parsed.inv_type.requires_header() {
                Err(format!(
                    "发票类型为「{}」，但在发票库中未找到对应发票（发票号码：{}）",
                    parsed.inv_type, inv_num
                ))
            } else {
                Ok(None)
            }
        }
    }
}

fn query_by_unq_key(conn: &Connection, key: &str) -> Result<Option<i64>, String> {
    conn.query_row(
        "SELECT id FROM invoice_headers WHERE inv_unq_key = ?1 LIMIT 1",
        rusqlite::params![key],
        |row| row.get(0),
    )
    .optional()
    .map_err(|e| format!("数据库查询失败：{}", e))
}

fn query_by_num(conn: &Connection, num: &str, total_fen: i64) -> Result<Option<i64>, String> {
    let mut stmt = conn
        .prepare("SELECT id, total FROM invoice_headers WHERE num = ?1")
        .map_err(|e| format!("数据库查询失败：{}", e))?;

    let results: Vec<(i64, i64)> = stmt
        .query_map(rusqlite::params![num], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
        })
        .map_err(|e| format!("数据库查询失败：{}", e))?
        .filter_map(|r| r.ok())
        .collect();

    match results.len() {
        0 => Ok(None),
        1 => Ok(Some(results[0].0)),
        _ => {
            // 多条：核对含税金额
            let matched: Vec<i64> = results
                .iter()
                .filter(|(_, db_total)| *db_total == total_fen)
                .map(|(id, _)| *id)
                .collect();

            match matched.len() {
                1 => Ok(Some(matched[0])),
                0 => Err(format!(
                    "发票号码「{}」在发票库中存在多条记录，但含税金额均不匹配（{}分），无法确定唯一发票",
                    num, total_fen
                )),
                _ => Err(format!(
                    "发票号码「{}」在发票库中存在多条含税金额相同的记录，无法确定唯一发票",
                    num
                )),
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────
// 核对税额和含税金额
// ─────────────────────────────────────────────────────────────
fn verify_amounts(
    conn: &Connection,
    parsed: &ParsedApVoucherRow,
    header_id: i64,
) -> Result<(), String> {
    let (db_tax, db_total): (i64, i64) = conn
        .query_row(
            "SELECT tax, total FROM invoice_headers WHERE id = ?1",
            rusqlite::params![header_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| format!("数据库查询失败：{}", e))?;

    if parsed.total != db_total {
        return Err(format!(
            "含税金额不一致：Excel 为 {}分，发票库为 {}分",
            parsed.total, db_total
        ));
    }

    // 税额为 0 时跳过税额核对
    if parsed.tax != 0 && parsed.tax != db_tax {
        return Err(format!(
            "税额不一致：Excel 为 {}分，发票库为 {}分",
            parsed.tax, db_tax
        ));
    }

    Ok(())
}

// ─────────────────────────────────────────────────────────────
// 核对增票的 inv_type（必须是 2、3、7 之一）
// ─────────────────────────────────────────────────────────────
fn verify_inv_type(conn: &Connection, header_id: i64) -> Result<(), String> {
    let inv_type: i32 = conn
        .query_row(
            "SELECT inv_type FROM invoice_headers WHERE id = ?1",
            rusqlite::params![header_id],
            |row| row.get(0),
        )
        .map_err(|e| format!("数据库查询失败：{}", e))?;

    if ![2, 3, 7].contains(&inv_type) {
        return Err(format!(
            "Excel 标注为增票，但发票库中 inv_type = {}，\
             不是增值税专用发票（期望值：2=数电专票 / 3=纸质专票 / 7=数电纸质专票）",
            inv_type
        ));
    }

    Ok(())
}

// ─────────────────────────────────────────────────────────────
// 构造 AP 专用错误（AP 专用，不放 utils）
// ─────────────────────────────────────────────────────────────
fn make_err(
    row: u32,
    barcode: Option<String>,
    inv_num: Option<String>,
    kind: &str,
    message: &str,
) -> ApVoucherImportError {
    ApVoucherImportError {
        row,
        barcode,
        inv_num,
        error_kind: kind.to_string(),
        message: message.to_string(),
    }
}
