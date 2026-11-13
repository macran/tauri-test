// src-tauri/src/commands/storage.rs
//
// 数据库容量与备份状态：标题栏的容量电池和系统管理里的“备份与同步”共用。

use std::{fs, path::Path};

use rusqlite::{Connection, OpenFlags};

use serde::Serialize;
use specta::Type;

use crate::{
    backup::{backup_all, last_backup_secs, pending_file_names},
    config::DB_SIZE_LIMIT_BYTES,
    state::{AppRuntimeState, CurrentLe, CurrentLeInner, DbState},
    sync::{checkpoint, is_share_available},
};

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct StorageStatus {
    /// 当前正在使用的库文件名
    pub db_file: String,
    /// 数据库文件大小（页数 × 页大小）。写入先进 WAL，只看主文件长度的话，批量导入后电池不会动
    #[specta(type = f64)]
    pub used_bytes: u64,
    #[specta(type = f64)]
    pub limit_bytes: u64,
    /// 共享盘此刻是否可访问（实时探测，不是启动时的快照）
    pub share_available: bool,
    /// 还没备份到共享盘的文件
    pub pending_files: Vec<String>,
    /// 最近一次全部备份成功的时间（秒级时间戳）；没成功过为空
    #[specta(type = Option<f64>)]
    pub last_backup_at: Option<u64>,
}

/// 数据库文件大小 = page_count × page_size（含空闲页，VACUUM 前不会缩小）。
/// 不用「主文件 + -wal」：WAL 不会自动缩短，而且同一页在 WAL 和主文件里各有一份，会重复计算。
/// 另开只读连接读，不抢业务连接的锁（导入期间业务连接被长时间占用）；打不开时退回主文件长度。
pub(crate) fn db_bytes(db: &Path) -> u64 {
    let pages = Connection::open_with_flags(db, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .and_then(|c| {
            c.query_row(
                "SELECT (SELECT page_count FROM pragma_page_count) * (SELECT page_size FROM pragma_page_size)",
                [],
                |r| r.get::<_, i64>(0),
            )
        })
        .ok();
    pages
        .map(|n| n as u64)
        .unwrap_or_else(|| fs::metadata(db).map(|m| m.len()).unwrap_or(0))
}

fn status_of(rt: &AppRuntimeState, current: &CurrentLeInner) -> StorageStatus {
    StorageStatus {
        db_file: current
            .db_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_string(),
        used_bytes: db_bytes(&current.db_path),
        limit_bytes: DB_SIZE_LIMIT_BYTES,
        share_available: is_share_available(&rt.shared_root),
        pending_files: pending_file_names(&rt.local_root),
        last_backup_at: last_backup_secs(&rt.local_root),
    }
}

#[tauri::command]
#[specta::specta]
pub async fn get_storage_status(
    app: tauri::State<'_, AppRuntimeState>,
    current: tauri::State<'_, CurrentLe>,
) -> Result<StorageStatus, String> {
    let rt = app.inner().clone();
    let cur = current.get();
    // 探测共享盘可能阻塞（网络盘断开时），放到阻塞线程里，不卡界面
    tauri::async_runtime::spawn_blocking(move || status_of(&rt, &cur))
        .await
        .map_err(|e| e.to_string())
}

/// 立即备份。先 checkpoint 把 WAL 并回主文件，再拷贝到共享盘。
///
/// 已知限制：checkpoint 之后、拷贝完成之前如果又有写入，这部分新数据在 WAL 里，不在被拷贝的主文件里，
/// 这次备份就不包含它。退出备份没有这个问题（没人再写）。所以这里只保证“到点击时刻为止”的数据。
/// 拷贝放在释放连接锁之后做，否则大库拷贝期间所有查询都会卡住。
#[tauri::command]
#[specta::specta]
pub async fn backup_now(
    app: tauri::State<'_, AppRuntimeState>,
    current: tauri::State<'_, CurrentLe>,
    db: tauri::State<'_, DbState>,
) -> Result<StorageStatus, String> {
    let rt = app.inner().clone();
    let cur = current.get();
    let conn = std::sync::Arc::clone(&db.conn);

    tauri::async_runtime::spawn_blocking(move || {
        {
            let guard = conn.lock().map_err(|e| format!("无法锁定数据库连接：{e}"))?;
            checkpoint(&guard).map_err(|e| format!("备份前 checkpoint 失败：{e}"))?;
        }
        // 返回的是告警（共享盘不可用 / 部分文件失败），不是致命错误：数据仍在本机，待备份台账已更新
        if let Some(warning) = backup_all(&rt) {
            return Err(warning.to_string());
        }
        Ok(status_of(&rt, &cur))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_is_page_count_times_page_size_and_tolerates_missing_file() {
        let dir = std::env::temp_dir().join(format!("storage_test_{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let db = dir.join("1784_2026.db");

        // 文件不存在：0，不报错，也不会凭空建出文件
        assert_eq!(db_bytes(&db), 0);
        assert!(!db.exists());

        let c = Connection::open(&db).unwrap();
        c.execute_batch("PRAGMA journal_mode = WAL; CREATE TABLE t(x BLOB); INSERT INTO t VALUES (zeroblob(100000));")
            .unwrap();
        let expect: i64 = c
            .query_row("SELECT page_count * page_size FROM pragma_page_count, pragma_page_size", [], |r| r.get(0))
            .unwrap();
        // 数据还在 WAL 里、主文件几乎是空的，也要算进去
        assert_eq!(db_bytes(&db), expect as u64);
        assert!(db_bytes(&db) > 100_000);

        drop(c);
        let _ = fs::remove_dir_all(&dir);
    }
}
