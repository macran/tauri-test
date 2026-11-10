// src-tauri/src/sync.rs
//
// 共享盘 → 本地的同步。
//
// 两个原则：
//   1. SQLite 永远只在本地文件上打开，不直接在网络共享盘上打开。
//   2. 是否需要复制由本地台账（sync-ledger.json）判断，不做哈希比较。
//      台账记录上次复制时共享盘文件的 len + mtime，两者都没变就跳过，
//      避免每次启动都从网络盘拉几百 MB。

use std::{
    collections::HashMap,
    fs,
    io,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use rusqlite::{Connection, OpenFlags};

use crate::{
    config::{ADMIN_DB_FILE, SYNC_LEDGER_FILE},
    errors::{AppError, AppResult},
};

// ─────────────────────────────────────────────────────────────
// 台账
// ─────────────────────────────────────────────────────────────

/// 一个文件在共享盘上的版本标记。
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FileStamp {
    pub len: u64,
    /// mtime，Unix 秒。取不到时为 0，此时视为需要复制。
    pub modified_secs: u64,
}

impl FileStamp {
    pub fn of(path: &Path) -> io::Result<Self> {
        let metadata = fs::metadata(path)?;
        let modified_secs = metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map(|duration| duration.as_secs())
            .unwrap_or(0);

        Ok(Self {
            len: metadata.len(),
            modified_secs,
        })
    }

    fn is_usable(&self) -> bool {
        self.modified_secs != 0
    }
}

/// 本地台账：文件名 → 上次成功复制时共享盘的版本标记。
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct SyncLedger {
    #[serde(default)]
    entries: HashMap<String, FileStamp>,
}

impl SyncLedger {
    pub fn load(local_root: &Path) -> Self {
        let path = local_root.join(SYNC_LEDGER_FILE);
        // 台账只是缓存，读失败就当成空台账，代价是本次多复制一遍。
        fs::read(path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, local_root: &Path) -> AppResult<()> {
        let path = local_root.join(SYNC_LEDGER_FILE);
        let bytes = serde_json::to_vec_pretty(self)?;
        fs::write(path, bytes)?;
        Ok(())
    }

    fn matches(&self, file_name: &str, remote: FileStamp) -> bool {
        remote.is_usable() && self.entries.get(file_name) == Some(&remote)
    }

    fn record(&mut self, file_name: &str, remote: FileStamp) {
        self.entries.insert(file_name.to_string(), remote);
    }
}

// ─────────────────────────────────────────────────────────────
// 通用复制
// ─────────────────────────────────────────────────────────────

/// 先复制到临时文件再改名，避免网络中断留下半个数据库。
pub fn copy_atomic(source: &Path, destination: &Path) -> io::Result<()> {
    let Some(parent) = destination.parent() else {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "目标路径没有父目录"));
    };
    fs::create_dir_all(parent)?;

    let file_name = destination
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("temp");
    let temporary = parent.join(format!(".{file_name}.part-{}", std::process::id()));

    fs::copy(source, &temporary)?;
    if destination.exists() {
        fs::remove_file(destination)?;
    }
    fs::rename(&temporary, destination).inspect_err(|_| {
        let _ = fs::remove_file(&temporary);
    })
}

/// 共享盘是否可访问。用一次目录读取判断，比 exists() 更可靠。
pub fn is_share_available(shared_root: &Path) -> bool {
    fs::read_dir(shared_root).is_ok()
}

// ─────────────────────────────────────────────────────────────
// admin.db
// ─────────────────────────────────────────────────────────────

/// 把共享盘上的 admin.db 同步到本地缓存，返回本地副本路径。
///
/// 顺序：共享盘可用且校验通过 → 用共享盘版本；否则回退到本地缓存；
/// 两边都没有才是 Fatal。
pub fn sync_admin_database(local_root: &Path, shared_root: &Path) -> AppResult<PathBuf> {
    let remote_path = shared_root.join(ADMIN_DB_FILE);
    let local_path = local_root.join(ADMIN_DB_FILE);

    if remote_path.exists() {
        match copy_admin_from_share(&remote_path, &local_path) {
            Ok(()) => return Ok(local_path),
            Err(error) if local_path.exists() => {
                eprintln!("共享盘 admin.db 不可用，改用本地缓存：{error}");
            }
            Err(error) => return Err(error),
        }
    }

    if local_path.exists() {
        validate_admin_database(&local_path)?;
        return Ok(local_path);
    }

    Err(AppError::admin_db_missing(&shared_root.display().to_string()))
}

fn copy_admin_from_share(remote_path: &Path, local_path: &Path) -> AppResult<()> {
    let incoming = local_path.with_extension("db.incoming");

    fs::copy(remote_path, &incoming)?;

    if let Err(error) = validate_admin_database(&incoming) {
        let _ = fs::remove_file(&incoming);
        return Err(error);
    }

    if local_path.exists() {
        fs::remove_file(local_path)?;
    }
    fs::rename(&incoming, local_path)?;
    Ok(())
}

/// admin.db 必须能打开、完整性检查通过、并且有 app_users 和 legal_entities。
pub fn validate_admin_database(path: &Path) -> AppResult<()> {
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|error| AppError::admin_db_invalid().with_detail(error))?;

    let integrity: String = conn
        .query_row("PRAGMA integrity_check;", [], |row| row.get(0))
        .map_err(|error| AppError::admin_db_invalid().with_detail(error))?;

    if integrity != "ok" {
        return Err(AppError::admin_db_invalid()
            .with_detail(format!("integrity_check 返回 {integrity}")));
    }

    let table_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master
             WHERE type = 'table'
               AND name IN ('app_users', 'legal_entities')",
            [],
            |row| row.get(0),
        )
        .map_err(|error| AppError::admin_db_invalid().with_detail(error))?;

    if table_count != 2 {
        return Err(AppError::admin_db_invalid()
            .with_detail("缺少 app_users 或 legal_entities 表"));
    }

    Ok(())
}

// ─────────────────────────────────────────────────────────────
// LE 业务数据库
// ─────────────────────────────────────────────────────────────

/// 拉取某个 LE 在共享盘上的所有数据库文件。
///
/// 返回本次实际复制的文件数。共享盘上没有该 LE 目录时返回 0，不算错误：
/// 说明这个 LE 还没有人用过，后面 init 会在本地建库。
pub fn pull_le_databases(
    local_root: &Path,
    shared_root: &Path,
    le: &str,
    ledger: &mut SyncLedger,
) -> AppResult<usize> {
    let remote_dir = shared_root.join(le);
    let entries = match fs::read_dir(&remote_dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(0),
        Err(error) => return Err(AppError::sync_failed(le).with_detail(error)),
    };

    let prefix = format!("{le}_");
    let mut copied = 0;

    for entry in entries {
        let source = entry.map_err(|error| AppError::sync_failed(le).with_detail(error))?.path();
        if !is_database_file(&source) {
            continue;
        }

        let Some(file_name) = source.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        if !file_name.starts_with(&prefix) {
            continue;
        }

        let destination = local_root.join(file_name);
        let remote_stamp = FileStamp::of(&source)
            .map_err(|error| AppError::sync_failed(le).with_detail(error))?;

        // 台账命中且本地文件还在，说明共享盘没有变化，跳过这次网络复制。
        if destination.exists() && ledger.matches(file_name, remote_stamp) {
            continue;
        }

        copy_atomic(&source, &destination).map_err(|error| {
            if is_locked(&error) {
                AppError::local_db_locked(file_name).with_detail(error)
            } else {
                AppError::sync_failed(le).with_detail(error)
            }
        })?;
        ledger.record(file_name, remote_stamp);
        copied += 1;
    }

    Ok(copied)
}

/// 修改时间相差在此范围内视为相同（共享盘文件系统时间精度可能较粗）。
const MTIME_TOLERANCE_SECS: u64 = 2;

fn is_locked(error: &io::Error) -> bool {
    // 5 拒绝访问 / 32 共享冲突 / 33 锁定冲突
    error.kind() == io::ErrorKind::PermissionDenied
        || matches!(error.raw_os_error(), Some(5 | 32 | 33))
}

/// 本地比共享盘新、且不在待备份台账里的文件：(本地, 共享盘)。
/// 这些文件说明上次未正常退出，直接拉取会覆盖本地改动，必须让用户决定。
/// 共享盘读取失败按“没有”处理（离线只告警，不阻止启动）。
pub fn find_local_newer(
    local_root: &Path,
    shared_root: &Path,
    le: &str,
    pending: &[String],
) -> Vec<(PathBuf, PathBuf)> {
    let Ok(entries) = fs::read_dir(shared_root.join(le)) else {
        return Vec::new();
    };
    let prefix = format!("{le}_");
    let mut result = Vec::new();

    for remote in entries.flatten().map(|entry| entry.path()) {
        let Some(name) = remote.file_name().and_then(|v| v.to_str()).map(str::to_owned) else {
            continue;
        };
        if !is_database_file(&remote) || !name.starts_with(&prefix) || pending.contains(&name) {
            continue;
        }
        let local = local_root.join(&name);
        let (Ok(remote_stamp), Ok(local_stamp)) = (FileStamp::of(&remote), FileStamp::of(&local))
        else {
            continue;
        };
        if remote_stamp.is_usable()
            && local_stamp.modified_secs > remote_stamp.modified_secs + MTIME_TOLERANCE_SECS
        {
            result.push((local, remote));
        }
    }
    result
}

/// 清理历史失败遗留的临时文件；正在使用的删除失败，直接忽略。
pub fn remove_stale_temp_files(local_root: &Path) {
    let Ok(entries) = fs::read_dir(local_root) else { return };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if name.starts_with('.') && (name.contains(".part-") || name.contains(".incoming-")) {
            let _ = fs::remove_file(entry.path());
        }
    }
}

/// 本地某个 LE 的所有年度数据库，返回 (路径, 年份)。
pub fn list_le_databases(local_root: &Path, le: &str) -> io::Result<Vec<(PathBuf, i32)>> {
    let prefix = format!("{le}_");
    let mut result = Vec::new();

    for entry in fs::read_dir(local_root)? {
        let path = entry?.path();
        if !is_database_file(&path) {
            continue;
        }

        let Some(stem) = path.file_stem().and_then(|value| value.to_str()) else {
            continue;
        };
        let Some(year_text) = stem.strip_prefix(&prefix) else {
            continue;
        };
        let Ok(file_year) = year_text.parse::<i32>() else {
            continue;
        };

        result.push((path, file_year));
    }

    result.sort_by_key(|(_, file_year)| *file_year);
    Ok(result)
}

fn is_database_file(path: &Path) -> bool {
    path.is_file() && path.extension().and_then(|value| value.to_str()) == Some("db")
}

/// WAL 落盘。备份前必须调用，否则共享盘上的副本会缺最近的写入。
pub fn checkpoint(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
}

/// 当前时间戳，用于备份台账里的记录时间。
pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}
