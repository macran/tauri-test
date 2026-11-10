// src-tauri/src/commands/le.rs
//
// 运行中切换 LE（法人实体）。流程等同于“重新打开软件”：
//   校验 → 收尾并备份当前库 → 判断共享盘 → 拉取/创建目标库 → 打开并检查 → 原地替换连接。
//
// 连接是原地替换的（锁住 DbState 后 *guard = 新连接），所以 54 处业务命令不用改；
// 周期性 checkpoint 线程持有的是同一个 Arc<Mutex<Connection>>，替换后自动作用在新连接上。
// 前端负责在切换成功后关闭所有标签页并清空缓存，页面上不会残留上一个 LE 的数据。

use std::sync::atomic::{AtomicBool, Ordering};

use rusqlite::Connection;
use serde::Serialize;
use specta::Type;
use tauri::{AppHandle, Emitter};

use crate::{
    backup::{backup_one, pending_file_names},
    errors::{AppError, AppResult},
    init::{ensure_le_database, init_tokenizer},
    state::{AppRuntimeState, CurrentLe, DbState, StartupProgress},
    sync::{checkpoint, find_local_newer, is_share_available, pull_le_databases, SyncLedger},
};
use crate::db::schema::init_db;

/// 切换进行中。同一时刻只允许一次，避免用户连点造成两个切换交叉。
static SWITCHING: AtomicBool = AtomicBool::new(false);

struct SwitchGuard;
impl SwitchGuard {
    fn acquire() -> Option<Self> {
        SWITCHING
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .ok()
            .map(|_| SwitchGuard)
    }
}
impl Drop for SwitchGuard {
    fn drop(&mut self) {
        SWITCHING.store(false, Ordering::Release);
    }
}

/// 前端用 listen 接收。进度是按阶段跳的：整库拷贝没有逐字节进度，不伪造平滑增长。
/// 命令返回即代表完成，所以不发 100%，避免“事件先到、命令后到”造成进度条先满后又报错。
const PROGRESS_EVENT: &str = "le-switch-progress";

fn progress(handle: &AppHandle, percent: u8, message: impl Into<String>) {
    // 发不出去不影响切换本身
    let _ = handle.emit(PROGRESS_EVENT, StartupProgress { percent, message: message.into() });
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SwitchableLes {
    /// 所有有效的 LE（admin.db 的 legal_entities.is_active = 1），没有按用户授权的限制
    pub les: Vec<String>,
    pub current: String,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SwitchResult {
    pub le: String,
    /// 不影响切换结果的提示：共享盘不可用、旧 LE 的备份未完成、新库是本地新建的等
    pub warnings: Vec<String>,
}

#[tauri::command]
#[specta::specta]
pub async fn list_switchable_les(
    app: tauri::State<'_, AppRuntimeState>,
    current: tauri::State<'_, CurrentLe>,
) -> Result<SwitchableLes, String> {
    Ok(SwitchableLes { les: app.active_le_codes.clone(), current: current.get().le })
}

#[tauri::command]
#[specta::specta]
pub async fn switch_le(
    le: String,
    handle: AppHandle,
    app: tauri::State<'_, AppRuntimeState>,
    current: tauri::State<'_, CurrentLe>,
    db: tauri::State<'_, DbState>,
) -> Result<SwitchResult, String> {
    let rt = app.inner().clone();
    let conn = std::sync::Arc::clone(&db.conn);
    let current = current.inner().clone();

    // 文件拷贝、完整性检查都可能很慢，放到阻塞线程，不卡界面
    tauri::async_runtime::spawn_blocking(move || switch_blocking(&handle, &rt, &current, &conn, le))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}

/// 准备 + 替换，全程持有切换守卫，两次切换不会在替换阶段交错。
fn switch_blocking(
    handle: &AppHandle,
    rt: &AppRuntimeState,
    current: &CurrentLe,
    conn: &std::sync::Arc<std::sync::Mutex<Connection>>,
    target: String,
) -> AppResult<SwitchResult> {
    let Some(_guard) = SwitchGuard::acquire() else {
        return Err(AppError::internal("正在切换 LE，请稍候"));
    };
    let old = current.get();
    let (db_path, warnings, new_conn) = prepare_switch(handle, rt, &old.le, &target, conn)?;

    // 持锁只做“替换”：连接和“当前 LE”在同一个区间里一起改，别的命令看不到半新半旧的状态
    let mut guard = conn
        .lock()
        .map_err(|error| AppError::internal(format!("无法锁定数据库连接：{error}")))?;
    let _ = checkpoint(&guard);
    *guard = new_conn;
    current.set(target.clone(), db_path);
    drop(guard);

    Ok(SwitchResult { le: target, warnings })
}

/// 锁外的全部准备工作。成功时返回新库路径、提示信息和已就绪的新连接。
fn prepare_switch(
    handle: &AppHandle,
    rt: &AppRuntimeState,
    old_le: &str,
    target: &str,
    conn: &std::sync::Arc<std::sync::Mutex<Connection>>,
) -> AppResult<(std::path::PathBuf, Vec<String>, Connection)> {
    if !rt.active_le_codes.iter().any(|l| l == target) {
        return Err(AppError::internal(format!("{target} 不是有效的 LE")));
    }
    if target == old_le {
        return Err(AppError::internal("已经是当前 LE"));
    }

    let mut warnings = Vec::new();

    progress(handle, 10, format!("正在保存 LE {old_le} 的数据…"));
    // 1. 收尾当前库：先 checkpoint，再备份。备份失败不阻止切换——数据在本机，待备份台账已记录
    if let Ok(guard) = conn.lock() {
        if let Err(error) = checkpoint(&guard) {
            warnings.push(format!("{old_le} 切换前 checkpoint 失败：{error}"));
        }
    }
    progress(handle, 30, format!("正在备份 LE {old_le}…"));
    if let Some(warning) = backup_one(rt, old_le) {
        warnings.push(format!("{old_le} 的改动尚未备份到共享盘：{warning}"));
    }

    // 2. 共享盘可用则拉取目标 LE；与启动一致，本地比共享盘新（上次没正常退出）时不能覆盖
    progress(handle, 55, format!("正在同步 LE {target} 的数据库…"));
    if is_share_available(&rt.shared_root) {
        let pending = pending_file_names(&rt.local_root);
        let local_newer = find_local_newer(&rt.local_root, &rt.shared_root, target, &pending);
        if !local_newer.is_empty() {
            let names: Vec<String> = local_newer
                .iter()
                .filter_map(|(local, _)| local.file_name()?.to_str().map(str::to_owned))
                .collect();
            return Err(AppError::local_newer(&names));
        }
        let mut ledger = SyncLedger::load(&rt.local_root);
        pull_le_databases(&rt.local_root, &rt.shared_root, target, &mut ledger)?;
        if let Err(error) = ledger.save(&rt.local_root) {
            eprintln!("写入同步台账失败：{error}");
        }
    } else {
        warnings.push("共享盘未连接，使用本机已有的数据；恢复后会自动同步备份".to_string());
    }

    // 3. 目标库：本地没有就新建（共享盘上也没有的新 LE），下次备份时会上传
    let db_path = ensure_le_database(handle, &rt.local_root, target)?;

    // 4. 打开并检查，与启动一致
    progress(handle, 75, format!("正在检查 LE {target} 的数据库…"));
    let mut new_conn = Connection::open(&db_path)
        .map_err(|error| AppError::business_db_unavailable(target).with_detail(error))?;
    let integrity: String = new_conn
        .query_row("PRAGMA integrity_check;", [], |row| row.get(0))
        .map_err(|error| AppError::business_db_unavailable(target).with_detail(error))?;
    if integrity != "ok" {
        return Err(AppError::database_corrupt(target, &integrity));
    }
    progress(handle, 90, "正在初始化…");
    // 遗留的加载：全文索引已改用自带的 trigram，不再依赖它，装不上只告警
    if let Err(error) = init_tokenizer(handle, &new_conn) {
        warnings.push(format!("中文分词扩展未加载（当前不影响检索）：{error}"));
    }
    init_db(&mut new_conn)
        .map_err(|error| AppError::business_db_unavailable(target).with_detail(error))?;

    Ok((db_path, warnings, new_conn))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn only_one_switch_at_a_time() {
        let first = SwitchGuard::acquire().expect("第一次应获得守卫");
        assert!(SwitchGuard::acquire().is_none(), "切换进行中，第二次必须被拒绝");
        drop(first);
        assert!(SwitchGuard::acquire().is_some(), "释放后可以再次切换");
    }

    #[test]
    fn current_le_is_shared_between_clones() {
        let a = CurrentLe::new("1618".into(), "1618_2026.db".into());
        let b = a.clone();
        b.set("2118".into(), "2118_2026.db".into());
        // 克隆出去的句柄（比如移进阻塞线程的那份）改了，原来的也要看到
        assert_eq!(a.get().le, "2118");
        assert_eq!(a.get().db_path.to_str(), Some("2118_2026.db"));
    }

    #[test]
    fn replacing_the_connection_in_place_is_seen_by_every_holder() {
        // 周期性 checkpoint 线程、业务命令拿的都是同一个 Arc<Mutex<Connection>>：
        // 原地替换后它们必须立刻看到新库，而不是继续操作旧库
        let old = Connection::open_in_memory().unwrap();
        old.execute_batch("CREATE TABLE t(v TEXT); INSERT INTO t VALUES ('旧LE');").unwrap();
        let shared = Arc::new(Mutex::new(old));
        let other_holder = Arc::clone(&shared);

        let new = Connection::open_in_memory().unwrap();
        new.execute_batch("CREATE TABLE t(v TEXT); INSERT INTO t VALUES ('新LE');").unwrap();
        *shared.lock().unwrap() = new;

        let v: String = other_holder.lock().unwrap().query_row("SELECT v FROM t", [], |r| r.get(0)).unwrap();
        assert_eq!(v, "新LE");
    }
}
