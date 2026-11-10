// src-tauri/src/commands/admin.rs
//
// 用户 / LE 管理。权威配置是共享盘上的 admin.db，所以这里直接读写共享盘那份，
// 成功后再把它复制回本地缓存（启动时本地缓存只在共享盘更新时才被覆盖）。
// 权限：每个命令都在事务内用 Windows 登录名重新查询 admin.db，必须是有效 leader；
// 不信任启动时缓存的 is_leader，也不接受前端传入的身份。

use std::fs;
use std::path::Path;

use rusqlite::{Connection, OpenFlags, TransactionBehavior};
use serde::Serialize;
use specta::Type;

use crate::db::admin_operations::{self as ops, AdminOverview, NewUser, UserPatch};
use crate::{AppRuntimeState, ADMIN_DB_FILE};

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CurrentUser {
    pub username: String,
    pub full_name: String,
    pub is_leader: bool,
}

/// 仅用于前端决定是否显示管理入口；真正的权限校验在每个管理命令里。
#[tauri::command]
#[specta::specta]
pub async fn current_user_info(app: tauri::State<'_, AppRuntimeState>) -> Result<CurrentUser, String> {
    Ok(CurrentUser {
        username: app.username.clone(),
        full_name: app.full_name.clone(),
        is_leader: app.is_leader,
    })
}

async fn admin_call<T: Send + 'static>(
    app: &AppRuntimeState,
    write: bool,
    f: impl FnOnce(&Connection, i64) -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    let rt = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let path = rt.shared_root.join(ADMIN_DB_FILE);
        let mut conn = Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_WRITE)
            .map_err(|e| format!("无法打开共享盘上的 admin.db，请检查网络连接：{e}"))?;
        conn.execute_batch("PRAGMA foreign_keys = ON; PRAGMA busy_timeout = 5000;")
            .map_err(|e| e.to_string())?;
        let behavior = if write { TransactionBehavior::Immediate } else { TransactionBehavior::Deferred };
        let tx = conn.transaction_with_behavior(behavior).map_err(|e| e.to_string())?;
        let caller = ops::require_leader(&tx, &rt.username)?;
        let out = f(&tx, caller)?;
        tx.commit().map_err(|e| e.to_string())?;
        drop(conn);
        if write {
            refresh_local_cache(&path, &rt.admin_db_path);
        }
        Ok(out)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// 尽力而为：本地副本只是启动时共享盘不可达的后备，复制失败不影响这次修改。
fn refresh_local_cache(shared: &Path, local: &Path) {
    let tmp = local.with_extension("db.refresh");
    let result = fs::copy(shared, &tmp).and_then(|_| fs::rename(&tmp, local));
    if let Err(e) = result {
        let _ = fs::remove_file(&tmp);
        eprintln!("刷新本地 admin.db 缓存失败：{e}");
    }
}

#[tauri::command]
#[specta::specta]
pub async fn admin_overview(app: tauri::State<'_, AppRuntimeState>) -> Result<AdminOverview, String> {
    admin_call(&app, false, |c, _| ops::overview(c)).await
}

#[tauri::command]
#[specta::specta]
pub async fn admin_add_user(
    user: NewUser,
    app: tauri::State<'_, AppRuntimeState>,
) -> Result<AdminOverview, String> {
    admin_call(&app, true, move |c, _| {
        ops::add_user(c, &user)?;
        ops::overview(c)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn admin_update_user(
    patch: UserPatch,
    app: tauri::State<'_, AppRuntimeState>,
) -> Result<AdminOverview, String> {
    admin_call(&app, true, move |c, caller| {
        ops::update_user(c, caller, &patch)?;
        ops::overview(c)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn admin_add_le(
    le_code: String,
    app: tauri::State<'_, AppRuntimeState>,
) -> Result<AdminOverview, String> {
    admin_call(&app, true, move |c, _| {
        ops::add_le(c, &le_code)?;
        ops::overview(c)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn admin_set_le_active(
    le_code: String,
    active: bool,
    app: tauri::State<'_, AppRuntimeState>,
) -> Result<AdminOverview, String> {
    admin_call(&app, true, move |c, _| {
        ops::set_le_active(c, &le_code, active)?;
        ops::overview(c)
    })
    .await
}

/// 改名只改 admin.db。LE 代码同时是数据库文件名（{LE}_{年}.db）和共享盘目录名，
/// 已经有数据库文件时改名会让数据与新代码脱节，所以直接拒绝。
#[tauri::command]
#[specta::specta]
pub async fn admin_rename_le(
    old_code: String,
    new_code: String,
    app: tauri::State<'_, AppRuntimeState>,
) -> Result<AdminOverview, String> {
    let (local_root, shared_root) = (app.local_root.clone(), app.shared_root.clone());
    admin_call(&app, true, move |c, _| {
        let has_local = crate::list_le_databases(&local_root, &old_code).map_err(|e| e.to_string())?;
        let has_shared = fs::read_dir(shared_root.join(&old_code)).is_ok_and(|mut d| d.next().is_some());
        if !has_local.is_empty() || has_shared {
            return Err(format!("LE {old_code} 已有数据库文件，改名会使历史数据与新代码脱节，不允许改名"));
        }
        ops::rename_le(c, &old_code, &new_code)?;
        ops::overview(c)
    })
    .await
}
