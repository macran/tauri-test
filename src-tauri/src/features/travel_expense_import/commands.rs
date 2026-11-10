// src-tauri/src/features/travel_expense_import/commands.rs
//
// Travel Expense 凭证文件导入 command。

use crate::error::AppError;
use crate::models::dto::ImportRequest;
use crate::DbState;

use super::models::TravelExpenseImportSummary;
use super::parser::parse_travel_expense_excel;
use super::repository::update_travel_expense_vouchers;

use rusqlite::Connection;
use std::sync::{Arc, Mutex};

#[tauri::command]
#[specta::specta]
pub async fn import_travel_expense_excel(
    request: ImportRequest,
    db: tauri::State<'_, DbState>,
) -> Result<TravelExpenseImportSummary, String> {
    let file_path = request.file_path.clone();
    let conn_arc = Arc::clone(&db.conn);

    tokio::task::spawn_blocking(move || run_travel_expense_import(&file_path, conn_arc))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e: AppError| e.to_string())
}

fn run_travel_expense_import(
    file_path: &str,
    conn_arc: Arc<Mutex<Connection>>,
) -> Result<TravelExpenseImportSummary, AppError> {
    // ── Step 1：解析 Excel ───────────────────────────────────
    let (parsed_rows, parse_errors) = parse_travel_expense_excel(file_path)?;

    // ── Step 2：获取数据库锁 ─────────────────────────────────
    let conn = conn_arc.lock().unwrap();

    // ── Step 3：开启事务 ─────────────────────────────────────
    conn.execute_batch("BEGIN IMMEDIATE")?;

    match update_travel_expense_vouchers(&conn, &parsed_rows, parse_errors) {
        Ok(mut summary) => {
            // 如果 operation 返回 committed=false，说明是业务前置条件失败，
            // 例如：认证清单尚未导入。
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
