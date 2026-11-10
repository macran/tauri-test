use crate::error::AppError;
use crate::DbState;

use super::models::{BankVoucherImportRequest, BankVoucherImportSummary};
use super::parser::parse_bank_voucher_excel;
use super::repository::insert_bank_vouchers;

use rusqlite::Connection;
use std::sync::{Arc, Mutex};

#[tauri::command]
#[specta::specta]
pub async fn import_bank_voucher_excel(
    request: BankVoucherImportRequest,
    db: tauri::State<'_, DbState>,
) -> Result<BankVoucherImportSummary, String> {
    let file_path = request.file_path.clone();
    let received_date = request.received_date.clone();
    let prepared_by = request.prepared_by.clone();
    if received_date.is_empty() {
        return Err("收到日期不能为空".to_string());
    }

    if prepared_by.is_empty() {
        return Err("制表人不能为空".to_string());
    }
    let conn_arc = Arc::clone(&db.conn);

    tokio::task::spawn_blocking(move || {
        run_bank_voucher_import(&file_path, &received_date, &prepared_by, conn_arc)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e: AppError| e.to_string())
}

fn run_bank_voucher_import(
    file_path: &str,
    received_date: &str,
    prepared_by: &str,
    conn_arc: Arc<Mutex<Connection>>,
) -> Result<BankVoucherImportSummary, AppError> {
    // ── Step 1：解析 Excel ───────────────────────────────────
    let (parsed_rows, parse_errors) = parse_bank_voucher_excel(file_path)?;

    // ── Step 2：获取数据库锁 ─────────────────────────────────
    let conn = conn_arc.lock().unwrap();

    // ── Step 3：开启事务 ─────────────────────────────────────
    conn.execute_batch("BEGIN IMMEDIATE")?;

    match insert_bank_vouchers(
        &conn,
        &parsed_rows,
        received_date,
        prepared_by,
        parse_errors,
    ) {
        Ok(mut summary) => {
            if !summary.committed {
                let _ = conn.execute_batch("ROLLBACK");
                return Ok(summary);
            }

            conn.execute_batch("COMMIT")?;
            summary.committed = true;
            Ok(summary)
        }
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK");
            Err(e.into())
        }
    }
}
