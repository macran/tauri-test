// src-tauri/src/features/ic_voucher_import/repository.rs
//
// 职责：
//   1. 按发票分组，逐组查找 header_id
//   2. 校验税额、价税合计
//   3. 批量插入 ic_vouchers
//   4. 整批回滚语义（由 command 层控制事务）

use rusqlite::Connection;

use super::models::{IcVoucherImportError, IcVoucherImportSummary, InvGroupSummary};

/// 在外部事务中调用，调用方负责 BEGIN IMMEDIATE / COMMIT / ROLLBACK。
pub fn insert_ic_vouchers(
    conn: &Connection,
    groups: Vec<InvGroupSummary>,
    received_date: &str,
    preparer: &str,
    mut errors: Vec<IcVoucherImportError>,
) -> Result<IcVoucherImportSummary, rusqlite::Error> {
    let mut inserted: u32 = 0;

    // ── 准备查询语句 ─────────────────────────────────────────
    // 通过 inv_unq_key 查找 header
    let mut stmt_by_unq = conn.prepare_cached(
        "SELECT id, tax, total FROM invoice_headers WHERE inv_unq_key = ?1 LIMIT 1",
    )?;

    // 通过 num 查找 header（发票代码为空且号码 < 20 位时兜底）
    let mut stmt_by_num =
        conn.prepare_cached("SELECT id, tax, total FROM invoice_headers WHERE num = ?1 LIMIT 1")?;

    // 通过 e_num 查找 header（20 位数电票 inv_unq_key 找不到时兜底）
    let mut stmt_by_enum =
        conn.prepare_cached("SELECT id, tax, total FROM invoice_headers WHERE e_num = ?1 LIMIT 1")?;

    // 插入语句
    let mut stmt_insert = conn.prepare_cached(
        "
        INSERT INTO ic_vouchers
            (header_id, legal_entity, billing, document_no, posted_date, original_po,
             received_date, preparer)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
        ",
    )?;

    // ── 逐组处理 ─────────────────────────────────────────────
    for group in groups {
        let first_row = group.rows.first().unwrap();
        let inv_num = &group.key.inv_num;
        let inv_code = &group.key.inv_code;
        let le = &group.key.legal_entity;

        // ── Step 1：查找 header_id ───────────────────────────
        let header_result = find_header(
            inv_num,
            inv_code.as_deref(),
            &mut stmt_by_unq,
            &mut stmt_by_num,
            &mut stmt_by_enum,
        );

        let (header_id, header_tax, header_total) = match header_result {
            Some(v) => v,
            None => {
                errors.push(IcVoucherImportError {
                    row: first_row.excel_row,
                    legal_entity: le.clone(),
                    inv_num: inv_num.clone(),
                    document_no: first_row.document_no.clone(),
                    error_kind: "发票未找到".to_string(),
                    message: format!("在发票库中找不到发票「{}」", inv_num),
                });
                continue;
            }
        };

        // ── Step 2：校验税额和价税合计（允许 ±1 分误差）──────
        let tax_diff = (group.total_tax_fen - header_tax).abs();
        let total_diff = (group.total_amount_fen - header_total).abs();

        // 税额、价税合计的误差都要收集，不能因税额有误而漏报价税合计
        let mut mismatch = false;

        if tax_diff > 1 {
            mismatch = true;
            errors.push(IcVoucherImportError {
                row: first_row.excel_row,
                legal_entity: le.clone(),
                inv_num: inv_num.clone(),
                document_no: first_row.document_no.clone(),
                error_kind: "税额不一致".to_string(),
                message: format!(
                    "税额不一致：Excel 汇总 {} 分，发票库 {} 分，差异 {} 分",
                    group.total_tax_fen, header_tax, tax_diff
                ),
            });
        }

        if total_diff > 1 {
            mismatch = true;
            errors.push(IcVoucherImportError {
                row: first_row.excel_row,
                legal_entity: le.clone(),
                inv_num: inv_num.clone(),
                document_no: first_row.document_no.clone(),
                error_kind: "价税合计不一致".to_string(),
                message: format!(
                    "价税合计不一致：Excel 汇总 {} 分，发票库 {} 分，差异 {} 分",
                    group.total_amount_fen, header_total, total_diff
                ),
            });
        }

        if mismatch {
            continue;
        }

        // ── Step 3：逐行插入 ic_vouchers ─────────────────────
        for row in &group.rows {
            match stmt_insert.execute(rusqlite::params![
                header_id,
                row.legal_entity,
                row.billing,
                row.document_no,
                row.posted_date,
                row.original_po,
                received_date,
                preparer,
            ]) {
                Ok(_) => inserted += 1,
                Err(rusqlite::Error::SqliteFailure(e, _))
                    if e.code == rusqlite::ErrorCode::ConstraintViolation =>
                {
                    errors.push(IcVoucherImportError {
                        row: row.excel_row,
                        legal_entity: row.legal_entity.clone(),
                        inv_num: inv_num.clone(),
                        document_no: row.document_no.clone(),
                        error_kind: "重复数据".to_string(),
                        message: format!(
                            "重复数据：发票「{}」+ Document「{}」已存在",
                            inv_num, row.document_no
                        ),
                    });
                }
                Err(e) => return Err(e),
            }
        }
    }

    // ── 有任何错误 → 整批回滚 ────────────────────────────────
    if !errors.is_empty() {
        let rollback_reason = format!("存在 {} 条错误，整批回滚，请修正后重新上传", errors.len());
        return Ok(IcVoucherImportSummary {
            inserted: 0,
            error_count: errors.len() as u32,
            errors,
            committed: false,
            rollback_reason: Some(rollback_reason),
        });
    }

    Ok(IcVoucherImportSummary {
        inserted,
        error_count: 0,
        errors: vec![],
        committed: true,
        rollback_reason: None,
    })
}

// ─────────────────────────────────────────────────────────────
// 查找 header：返回 (id, tax_fen, total_fen)
// ─────────────────────────────────────────────────────────────
fn find_header(
    inv_num: &str,
    inv_code: Option<&str>,
    stmt_by_unq: &mut rusqlite::CachedStatement<'_>,
    stmt_by_num: &mut rusqlite::CachedStatement<'_>,
    stmt_by_enum: &mut rusqlite::CachedStatement<'_>,
) -> Option<(i64, i64, i64)> {
    let num_len = inv_num.trim().len();

    if num_len >= 20 {
        // ── 数电票：先 inv_unq_key，再 e_num ─────────────────
        let by_unq = stmt_by_unq
            .query_row(rusqlite::params![inv_num.trim()], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, i64>(2)?,
                ))
            })
            .ok();

        if by_unq.is_some() {
            return by_unq;
        }

        // 兜底：e_num
        stmt_by_enum
            .query_row(rusqlite::params![inv_num.trim()], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, i64>(2)?,
                ))
            })
            .ok()
    } else {
        // ── 传统发票 ─────────────────────────────────────────
        match inv_code {
            Some(code) if !code.trim().is_empty() => {
                // 有发票代码：用 code-num 拼接查 inv_unq_key
                let unq_key = format!("{}-{}", code.trim(), inv_num.trim());
                stmt_by_unq
                    .query_row(rusqlite::params![unq_key], |r| {
                        Ok((
                            r.get::<_, i64>(0)?,
                            r.get::<_, i64>(1)?,
                            r.get::<_, i64>(2)?,
                        ))
                    })
                    .ok()
            }
            _ => {
                // 无发票代码：直接用 num 字段查
                stmt_by_num
                    .query_row(rusqlite::params![inv_num.trim()], |r| {
                        Ok((
                            r.get::<_, i64>(0)?,
                            r.get::<_, i64>(1)?,
                            r.get::<_, i64>(2)?,
                        ))
                    })
                    .ok()
            }
        }
    }
}
