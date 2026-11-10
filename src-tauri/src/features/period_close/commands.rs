// 状态 / 取消确认 / 锁定 / 管理员解锁 / 重新锁定。操作人取自 AppRuntimeState.username，前端不传、也无法伪造。
// 「确认」要写各模块自己的已确认明细快照，在 input_tax_transfer / stamp_tax 的命令里。
// 解锁要管理员：每次都实时读共享盘 admin.db，不信任启动时缓存的 is_leader。

use chrono::Local;
use rusqlite::{Connection, OpenFlags};
use tauri::State;

use super::models::{LockRequest, PeriodActionRequest, PeriodStatus, UnlockRequest};
use super::repository as ops;
use crate::features::admin::repository as admin;
use crate::{config::ADMIN_DB_FILE, AppRuntimeState, DbState};

pub(crate) fn now() -> String {
    Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

pub(crate) fn is_leader_now(app: &AppRuntimeState) -> bool {
    let path = app.shared_root.join(ADMIN_DB_FILE);
    Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .ok()
        .is_some_and(|conn| admin::require_leader(&conn, &app.username).is_ok())
}

pub(crate) fn lock_db<'a>(
    db: &'a State<'_, DbState>,
) -> Result<std::sync::MutexGuard<'a, Connection>, String> {
    db.conn.lock().map_err(|_| "数据库连接锁定失败".to_string())
}

/// 取得已存在的 run；还没点过「匹配」时报错。
fn existing_run(
    tx: &rusqlite::Transaction<'_>,
    module: &str,
    filing_from: &str,
    filing_to: &str,
) -> Result<ops::RunRow, String> {
    ops::find_run(tx, module, filing_from, filing_to)?
        .ok_or_else(|| format!("纳税所属期 {} 还没有匹配结果", ops::label(filing_from, filing_to)))
}

#[tauri::command]
#[specta::specta]
pub fn get_period_status(
    request: PeriodActionRequest,
    db: State<'_, DbState>,
    app: State<'_, AppRuntimeState>,
) -> Result<PeriodStatus, String> {
    ops::tables(&request.module)?;
    let (from, to) = ops::validate_filing(&request.filing_from, &request.filing_to)?;
    let mut conn = lock_db(&db)?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    // 状态栏只需要提示，不为此访问共享盘；真正的权限在 unlock_tax_period 里校验
    let status = ops::status(&tx, &request.module, &from, &to, app.is_leader)?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(status)
}

/// 取消确认：批次回到待确认，可继续匹配、修改；匹配结果和人工修改原样保留。已锁定的要先由管理员解锁。
#[tauri::command]
#[specta::specta]
pub fn unconfirm_tax_period(
    request: PeriodActionRequest,
    db: State<'_, DbState>,
    app: State<'_, AppRuntimeState>,
) -> Result<PeriodStatus, String> {
    ops::tables(&request.module)?;
    let (from, to) = ops::validate_filing(&request.filing_from, &request.filing_to)?;
    let mut conn = lock_db(&db)?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let run = existing_run(&tx, &request.module, &from, &to)?;
    ops::unconfirm(&tx, &run)?;
    let status = ops::status(&tx, &request.module, &from, &to, app.is_leader)?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(status)
}

/// 锁定：填写本批进项转出 / 印花税自己的会计凭证号、入账日期、做账人。锁定后永久只读。
#[tauri::command]
#[specta::specta]
pub fn lock_tax_period(
    request: LockRequest,
    db: State<'_, DbState>,
    app: State<'_, AppRuntimeState>,
) -> Result<PeriodStatus, String> {
    ops::tables(&request.module)?;
    let (from, to) = ops::validate_filing(&request.filing_from, &request.filing_to)?;
    let mut conn = lock_db(&db)?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let run = existing_run(&tx, &request.module, &from, &to)?;
    ops::lock(
        &tx,
        &run,
        &request.voucher_no,
        &request.posted_date,
        &request.preparer,
        &app.username,
        &now(),
    )?;
    let status = ops::status(&tx, &request.module, &from, &to, app.is_leader)?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(status)
}

/// 管理员解锁任意纳税所属期：新开补充批次，旧批次不动。
#[tauri::command]
#[specta::specta]
pub fn unlock_tax_period(
    request: UnlockRequest,
    db: State<'_, DbState>,
    app: State<'_, AppRuntimeState>,
) -> Result<PeriodStatus, String> {
    ops::tables(&request.module)?;
    let (from, to) = ops::validate_filing(&request.filing_from, &request.filing_to)?;
    if !is_leader_now(&app) {
        return Err("只有管理员才能解锁".to_string());
    }
    let mut conn = lock_db(&db)?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let run = existing_run(&tx, &request.module, &from, &to)?;
    ops::unlock(&tx, &run, &request.reason, &app.username, &now())?;
    let status = ops::status(&tx, &request.module, &from, &to, true)?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(status)
}

/// 管理员重新锁定：补充批次没有新增明细时，撤销这个空批次，回到锁定状态。
#[tauri::command]
#[specta::specta]
pub fn relock_tax_period(
    request: PeriodActionRequest,
    db: State<'_, DbState>,
    app: State<'_, AppRuntimeState>,
) -> Result<PeriodStatus, String> {
    ops::tables(&request.module)?;
    let (from, to) = ops::validate_filing(&request.filing_from, &request.filing_to)?;
    if !is_leader_now(&app) {
        return Err("只有管理员才能重新锁定".to_string());
    }
    let mut conn = lock_db(&db)?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let run = existing_run(&tx, &request.module, &from, &to)?;
    // 补充批次里还有未确认的结果或人工修改，就不能当作「没有新增」直接撤销
    let pending = ops::unconfirmed_count(&tx, &request.module, run.id)?;
    ops::relock(&tx, &run, pending)?;
    let status = ops::status(&tx, &request.module, &from, &to, true)?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(status)
}
