use super::models::*;
use super::red::{query_links, query_red};
use super::repository::query_reversal;
use crate::DbState;

/// 红冲查询入口：按 view 选主体，并总是附带对应关系。
pub fn run(conn: &rusqlite::Connection, f: &ReversalQueryFilter) -> rusqlite::Result<ReversalQueryResult> {
    let mut r = if f.view.as_deref() == Some("red") {
        query_red(conn, f)?
    } else {
        query_reversal(conn, f)?
    };
    let l = query_links(conn, f)?;
    r.links = l.links;
    r.link_total = l.link_total;
    Ok(r)
}

#[tauri::command]
#[specta::specta]
pub async fn query_reversal_overview(
    filter: ReversalQueryFilter,
    db: tauri::State<'_, DbState>,
) -> Result<ReversalQueryResult, String> {
    let conn = db.conn.lock().unwrap();
    run(&conn, &filter).map_err(|e| e.to_string())
}
