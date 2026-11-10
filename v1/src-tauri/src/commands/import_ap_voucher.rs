// src-tauri/src/commands/ap_voucher_commands.rs
//
// AP Voucher 凭证文件导入 command。

use crate::db::ap_voucher_operations::import_ap_vouchers;
use crate::error::AppError;
use crate::models::dto::{ApVoucherImportRequest, ApVoucherImportSummary};
use crate::parser::ap_voucher::parse_ap_voucher_excel;
use crate::DbState;

use rusqlite::Connection;
use std::sync::{Arc, Mutex};

#[tauri::command]
#[specta::specta]
pub async fn import_ap_voucher_excel(
    request: ApVoucherImportRequest,
    db: tauri::State<'_, DbState>,
) -> Result<ApVoucherImportSummary, String> {
    let file_path = request.file_path.clone();
    let received_date = request.received_date.clone();
    let preparer = request.preparer.clone();
    let conn_arc = Arc::clone(&db.conn);

    tokio::task::spawn_blocking(move || {
        run_ap_voucher_import(&file_path, &received_date, &preparer, conn_arc)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e: AppError| e.to_string())
}

fn run_ap_voucher_import(
    file_path: &str,
    received_date: &str,
    preparer: &str,
    conn_arc: Arc<Mutex<Connection>>,
) -> Result<ApVoucherImportSummary, AppError> {
    // ── Step 1：解析 Excel ───────────────────────────────────
    let (parsed_rows, parse_errors) = parse_ap_voucher_excel(file_path)?;

    // ── Step 2：获取数据库锁 ─────────────────────────────────
    let conn = conn_arc.lock().unwrap();

    // ── Step 3：开启事务 ─────────────────────────────────────
    conn.execute_batch("BEGIN IMMEDIATE")?;

    match import_ap_vouchers(&conn, parsed_rows, received_date, preparer, parse_errors) {
        Ok(mut summary) => {
            // 如果 operation 返回 committed=false，说明是业务前置条件失败
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
