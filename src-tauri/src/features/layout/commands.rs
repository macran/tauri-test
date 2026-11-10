// src-tauri/src/commands/layout.rs
//
// 结果表格列布局的读取 / 保存 / 设默认 / 删除。当前用户取自 AppRuntimeState.username，
// 前端不传、也无法伪造所有者。

use super::repository::{self as ops, LayoutRecord};
use crate::{AppRuntimeState, DbState};

#[tauri::command]
#[specta::specta]
pub async fn list_layouts(
    scope: String,
    db: tauri::State<'_, DbState>,
    app: tauri::State<'_, AppRuntimeState>,
) -> Result<Vec<LayoutRecord>, String> {
    let conn = db.conn.lock().unwrap();
    ops::list_layouts(&conn, &scope, &app.username).map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn save_layout(
    scope: String,
    name: String,
    layout: String,
    is_private: bool,
    db: tauri::State<'_, DbState>,
    app: tauri::State<'_, AppRuntimeState>,
) -> Result<u32, String> {
    let conn = db.conn.lock().unwrap();
    ops::save_layout(&conn, &scope, &app.username, name.trim(), &layout, is_private)
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn set_default_layout(
    scope: String,
    id: Option<u32>,
    db: tauri::State<'_, DbState>,
    app: tauri::State<'_, AppRuntimeState>,
) -> Result<(), String> {
    let mut conn = db.conn.lock().unwrap();
    ops::set_default_layout(&mut conn, &scope, &app.username, id)
}

#[tauri::command]
#[specta::specta]
pub async fn delete_layout(
    id: u32,
    db: tauri::State<'_, DbState>,
    app: tauri::State<'_, AppRuntimeState>,
) -> Result<(), String> {
    let conn = db.conn.lock().unwrap();
    ops::delete_layout(&conn, &app.username, id)
}
