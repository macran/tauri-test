// src-tauri/src/backup.rs
//
// 本地 → 共享盘的备份。
//
// 退出时的原则：不能因为网络盘不可用就卡住退出，也不能默默丢数据。
// 做法是把没备份成功的文件写进 backup-pending.json：
//   - 下次启动时优先补备份；
//   - 运行期间后台线程按间隔重试；
//   - 前端可以读到待备份数量并显示提示。

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    thread,
    time::Duration,
};

use tauri::Manager;

use crate::{
    config::{BACKUP_LEDGER_FILE, BACKUP_RETRY_INTERVAL_SECONDS, LAST_BACKUP_FILE},
    errors::{AppError, AppResult},
    state::{AppRuntimeState, DbState},
    sync::{checkpoint, copy_atomic, is_share_available, list_le_databases, now_secs},
};

// ─────────────────────────────────────────────────────────────
// 待备份台账
// ─────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct PendingBackups {
    /// 本地文件名 → 首次记录待备份的时间戳。
    #[serde(default)]
    entries: BTreeMap<String, u64>,
}

impl PendingBackups {
    pub fn load(local_root: &Path) -> Self {
        fs::read(local_root.join(BACKUP_LEDGER_FILE))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, local_root: &Path) -> AppResult<()> {
        let bytes = serde_json::to_vec_pretty(self)?;
        fs::write(local_root.join(BACKUP_LEDGER_FILE), bytes)?;
        Ok(())
    }

    pub fn mark(&mut self, file_name: &str) {
        self.entries.entry(file_name.to_string()).or_insert_with(now_secs);
    }

    pub fn clear(&mut self, file_name: &str) {
        self.entries.remove(file_name);
    }

    pub fn file_names(&self) -> Vec<String> {
        self.entries.keys().cloned().collect()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

// ─────────────────────────────────────────────────────────────
// 备份执行
// ─────────────────────────────────────────────────────────────

/// 备份某个 LE 的所有本地数据库到共享盘。
/// 单个文件失败不会中断其他文件，失败的会留在 pending 里。
fn backup_le_databases(
    local_root: &Path,
    shared_root: &Path,
    le: &str,
    pending: &mut PendingBackups,
) -> AppResult<()> {
    let databases = list_le_databases(local_root, le)?;
    if databases.is_empty() {
        return Ok(());
    }

    let remote_dir = shared_root.join(le);
    fs::create_dir_all(&remote_dir)
        .map_err(|error| AppError::backup_failed(error))?;

    let mut first_error = None;

    for (source, _) in databases {
        let Some(file_name) = source.file_name().and_then(|value| value.to_str()) else {
            continue;
        };

        match copy_atomic(&source, &remote_dir.join(file_name)) {
            Ok(()) => pending.clear(file_name),
            Err(error) => {
                pending.mark(file_name);
                eprintln!("备份 {file_name} 失败：{error}");
                first_error.get_or_insert(error);
            }
        }
    }

    match first_error {
        Some(error) => Err(AppError::backup_failed(error)),
        None => Ok(()),
    }
}

/// 最近一次全部备份成功的时间（秒级时间戳）。没有成功过返回 None。
pub fn last_backup_secs(local_root: &Path) -> Option<u64> {
    fs::read_to_string(local_root.join(LAST_BACKUP_FILE)).ok()?.trim().parse().ok()
}

fn record_last_backup(local_root: &Path) {
    // 记录失败不影响备份本身，下次成功时会再写
    let _ = fs::write(local_root.join(LAST_BACKUP_FILE), now_secs().to_string());
}

/// 备份所有 active LE。返回失败时的告警，供前端提示。
pub fn backup_all(runtime: &AppRuntimeState) -> Option<AppError> {
    let mut pending = PendingBackups::load(&runtime.local_root);

    if !is_share_available(&runtime.shared_root) {
        // 共享盘不可用时不要逐个 LE 试一遍，直接把本地文件全部标成待备份。
        mark_all_pending(runtime, &mut pending);
        let _ = pending.save(&runtime.local_root);
        return Some(AppError::backup_pending(&pending.file_names()));
    }

    let mut failure = None;
    for le in &runtime.active_le_codes {
        if let Err(error) = backup_le_databases(
            &runtime.local_root,
            &runtime.shared_root,
            le,
            &mut pending,
        ) {
            failure.get_or_insert(error);
        }
    }

    let _ = pending.save(&runtime.local_root);

    if failure.is_some() && !pending.is_empty() {
        return Some(AppError::backup_pending(&pending.file_names()));
    }
    if failure.is_none() {
        record_last_backup(&runtime.local_root);
    }
    failure
}

/// 只备份一个 LE（切换 LE 前收尾用）。共享盘不可用或失败时返回告警，数据留在本机、待备份台账已记录。
pub fn backup_one(runtime: &AppRuntimeState, le: &str) -> Option<AppError> {
    let mut pending = PendingBackups::load(&runtime.local_root);

    if !is_share_available(&runtime.shared_root) {
        if let Ok(databases) = list_le_databases(&runtime.local_root, le) {
            for (path, _) in databases {
                if let Some(name) = path.file_name().and_then(|v| v.to_str()) {
                    pending.mark(name);
                }
            }
        }
        let _ = pending.save(&runtime.local_root);
        return Some(AppError::backup_pending(&pending.file_names()));
    }

    let result = backup_le_databases(&runtime.local_root, &runtime.shared_root, le, &mut pending);
    let _ = pending.save(&runtime.local_root);
    result.err()
}

fn mark_all_pending(runtime: &AppRuntimeState, pending: &mut PendingBackups) {
    for le in &runtime.active_le_codes {
        let Ok(databases) = list_le_databases(&runtime.local_root, le) else {
            continue;
        };
        for (path, _) in databases {
            if let Some(file_name) = path.file_name().and_then(|value| value.to_str()) {
                pending.mark(file_name);
            }
        }
    }
}

/// 启动时补上一次没做完的备份。返回告警而不是错误：补备份失败不影响使用。
pub fn retry_pending_backups(runtime: &AppRuntimeState) -> Option<AppError> {
    let pending = PendingBackups::load(&runtime.local_root);
    if pending.is_empty() {
        return None;
    }

    if !is_share_available(&runtime.shared_root) {
        return Some(AppError::backup_pending(&pending.file_names()));
    }

    backup_all(runtime)
}

/// 退出前调用：先 checkpoint 再备份。
pub fn backup_on_exit(app: &tauri::AppHandle) {
    let Some(runtime) = app.try_state::<AppRuntimeState>() else {
        // 启动阶段就失败的情况，没有 runtime，也没有需要备份的数据。
        return;
    };

    if let Some(db_state) = app.try_state::<DbState>() {
        match db_state.conn.lock() {
            Ok(conn) => {
                if let Err(error) = checkpoint(&conn) {
                    eprintln!("退出前 checkpoint 失败：{error}");
                }
            }
            Err(error) => eprintln!("退出时无法锁定数据库连接：{error}"),
        }
    }

    if let Some(warning) = backup_all(&runtime) {
        eprintln!("退出备份未完成：{warning}");
    }
}

/// 运行期间的后台重试线程。共享盘恢复后会自动补上备份。
pub fn spawn_retry_worker(runtime: AppRuntimeState) {
    thread::spawn(move || {
        let interval = Duration::from_secs(BACKUP_RETRY_INTERVAL_SECONDS);
        loop {
            thread::sleep(interval);

            let pending = PendingBackups::load(&runtime.local_root);
            if pending.is_empty() || !is_share_available(&runtime.shared_root) {
                continue;
            }

            if let Some(warning) = backup_all(&runtime) {
                eprintln!("后台补备份未完成：{warning}");
            }
        }
    });
}

/// 待备份文件列表，供前端显示。
pub fn pending_file_names(local_root: &PathBuf) -> Vec<String> {
    PendingBackups::load(local_root).file_names()
}
