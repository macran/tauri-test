// src-tauri/src/features/ic_voucher_import/commands.rs

use crate::error::AppError;
use crate::DbState;

use super::models::{IcVoucherImportRequest, IcVoucherImportSummary};
use super::parser::parse_ic_voucher_excel;
use super::repository::insert_ic_vouchers;

use rusqlite::Connection;
use std::sync::{Arc, Mutex};

#[tauri::command]
#[specta::specta]
pub async fn import_ic_voucher_excel(
    request: IcVoucherImportRequest,
    db: tauri::State<'_, DbState>,
) -> Result<IcVoucherImportSummary, String> {
    let file_path = request.file_path.clone();
    let received_date = request.received_date.clone();
    let preparer = request.preparer.clone();
    let conn_arc = Arc::clone(&db.conn);

    tokio::task::spawn_blocking(move || {
        run_ic_voucher_import(&file_path, &received_date, &preparer, conn_arc)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e: AppError| e.to_string())
}

fn run_ic_voucher_import(
    file_path: &str,
    received_date: &str,
    preparer: &str,
    conn_arc: Arc<Mutex<Connection>>,
) -> Result<IcVoucherImportSummary, AppError> {
    // ── Step 1：解析 Excel ───────────────────────────────────
    let (groups, parse_errors) = parse_ic_voucher_excel(file_path)?;

    // ── Step 2：获取数据库锁 ─────────────────────────────────
    let conn = conn_arc.lock().unwrap();

    // ── Step 3：开启事务 ─────────────────────────────────────
    conn.execute_batch("BEGIN IMMEDIATE")?;

    match insert_ic_vouchers(&conn, groups, received_date, preparer, parse_errors) {
        Ok(mut summary) => {
            if !summary.committed {
                // 有错误，整批回滚
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
