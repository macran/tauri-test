// 在你现有的 commands 模块中添加

use crate::db::operations_certified::insert_certified_list;
use crate::error::AppError;
use crate::models::dto::{CertifiedImportSummary, ImportRequest};
use crate::parser::certified::parse_certified_excel;
use crate::DbState;
use rusqlite::Connection;
use std::sync::{Arc, Mutex};

#[tauri::command]
#[specta::specta]

pub async fn import_certified_excel(
    request: ImportRequest,
    db: tauri::State<'_, DbState>,
) -> Result<CertifiedImportSummary, String> {
    let file_path = request.file_path.clone();
    let conn_arc = Arc::clone(&db.conn);

    tokio::task::spawn_blocking(move || run_certified_import(&file_path, conn_arc))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e: AppError| e.to_string())
}

fn run_certified_import(
    file_path: &str,
    conn_arc: Arc<Mutex<Connection>>,
) -> Result<CertifiedImportSummary, AppError> {
    // ── 解析 Excel ────────────────────────────────────────────
    let (tax_period, parsed_rows, parse_errors) = parse_certified_excel(file_path)?;

    // ── 获取锁 ────────────────────────────────────────────────
    let conn = conn_arc.lock().unwrap();

    // ── 事务写入 ──────────────────────────────────────────────
    conn.execute_batch("BEGIN IMMEDIATE")?;

    match insert_certified_list(&conn, &parsed_rows, &tax_period, parse_errors) {
        Ok(mut summary) => {
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
