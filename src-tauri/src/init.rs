// src-tauri/src/init.rs
//
// 启动初始化的唯一入口。
//
// 这里的每一步失败都要转换成 AppError，绝不 unwrap、绝不 panic：
// setup 里只负责把结果写进 StartupState，窗口始终能打开。
//
// 顺序不能改：
//   1. 定位 %LOCALAPPDATA% 缓存目录
//   2. 补上一次没做完的备份（此时还没有人写数据库，最安全）
//   3. 同步并校验 admin.db
//   4. 读取当前 Windows 用户与 active LE 列表
//   5. 拉取所有 active LE 的数据库（失败只告警）
//   6. 打开当前 LE 的数据库、装 tokenizer、建表

use std::path::{Path, PathBuf};

use chrono::{Datelike, Local};
use rusqlite::{Connection, OpenFlags, OptionalExtension};
use tauri::{AppHandle, Manager};

use crate::{
    backup,
    config::{DB_SIZE_LIMIT_BYTES, LOCAL_DB_DIR, SHARED_ROOT},
    db::schema::init_db,
    errors::{AppError, AppResult, ErrorCode},
    state::{AdminUser, AppRuntimeState, StartupState},
    sync::{
        self, checkpoint, find_local_newer, is_share_available, list_le_databases,
        pull_le_databases, remove_stale_temp_files, SyncLedger,
    },
};

/// 推送启动进度：事件给已挂载的前端，StartupState 给晚挂载的前端轮询。
///
/// 进度轴是后端与 main 窗口共用的：后端初始化占 0..=60，main 窗口加载占 60..=100，
/// 100 即 main 可以显示（见 commands::startup::report_main_progress）。
/// 这里传入的是后端自己的 0..=100，按 60% 的比例压进共用轴。
fn progress(app: &AppHandle, percent: u8, message: &str) {
    crate::commands::startup::publish_progress(app, (u16::from(percent) * 60 / 100) as u8, message);
}

/// 初始化成功的产物。conn 交给 DbState，runtime 交给 AppRuntimeState。
pub struct Initialized {
    pub conn: Connection,
    pub runtime: AppRuntimeState,
    /// 不致命但需要告诉用户的问题，例如共享盘离线、某个 LE 没同步上。
    pub warnings: Vec<AppError>,
}

pub fn initialize(app: &AppHandle) -> AppResult<Initialized> {
    let mut warnings = Vec::new();

    progress(app, 5, "正在定位本地数据目录…");
    // 1. 本地缓存目录
    let local_root = dirs::data_local_dir()
        .ok_or_else(AppError::local_dir_unavailable)?
        .join(LOCAL_DB_DIR);
    std::fs::create_dir_all(&local_root).map_err(|error| {
        AppError::local_dir_unavailable().with_detail(error)
    })?;

    remove_stale_temp_files(&local_root);

    progress(app, 15, "正在检测共享盘连接…");
    let shared_root = PathBuf::from(SHARED_ROOT);
    let share_available = is_share_available(&shared_root);
    if !share_available {
        warnings.push(AppError::share_unavailable(&shared_root.display().to_string()));
    }

    // 3. admin.db
    progress(app, 30, "正在同步用户配置…");
    let admin_db_path = sync::sync_admin_database(&local_root, &shared_root)?;

    // 4. 用户与 LE
    progress(app, 45, "正在读取用户与法人实体…");
    let username = detect_username();
    let user = load_user(&admin_db_path, &username)?;
    let active_le_codes = load_active_le_codes(&admin_db_path)?;

    if active_le_codes.is_empty() {
        return Err(AppError::no_active_legal_entity());
    }

    if let Some(default_le) = user.default_le_code.as_deref() {
        if !active_le_codes.iter().any(|le| le == default_le) {
            return Err(AppError::default_legal_entity_invalid(&user.username, default_le));
        }
    }

    // 5. 同步各 LE 的数据库。共享盘离线时不必逐个重试。
    // 当前 LE 若本地比共享盘新（上次未正常退出）必须停下来让用户决定，否则拉取会覆盖本地改动。
    let dashboard_mode = user.default_le_code.is_none();
    let active_le = user
        .default_le_code
        .clone()
        .unwrap_or_else(|| active_le_codes[0].clone());

    if share_available {
        let pending = backup::pending_file_names(&local_root);
        let local_newer = find_local_newer(&local_root, &shared_root, &active_le, &pending);
        if !local_newer.is_empty() {
            let names: Vec<String> = local_newer
                .iter()
                .filter_map(|(local, _)| local.file_name()?.to_str().map(str::to_owned))
                .collect();
            if let Some(state) = app.try_state::<StartupState>() {
                state.set_pending_upload(local_newer);
            }
            return Err(AppError::local_newer(&names));
        }

        let mut ledger = SyncLedger::load(&local_root);
        let total = active_le_codes.len();
        for (index, le) in active_le_codes.iter().enumerate() {
            let percent = 50 + (25 * index / total) as u8;
            progress(app, percent, &format!("正在同步 {le} 的数据库…"));
            match pull_le_databases(&local_root, &shared_root, le, &mut ledger) {
                Ok(_) => {}
                // 当前 LE 的本地库被占用无法更新：继续会读到旧数据，必须拦下。
                Err(error) if error.code == ErrorCode::LocalDbLocked && *le == active_le => {
                    return Err(error)
                }
                Err(error) => warnings.push(error),
            }
        }
        if let Err(error) = ledger.save(&local_root) {
            eprintln!("写入同步台账失败：{error}");
        }
    }

    // 6. 当前 LE 的数据库
    // default_le_code 为 NULL 表示全 LE 汇总视图，此时用第一个 active LE 作为写入库。
    progress(app, 80, "正在打开当前数据库…");
    let active_db_path = ensure_le_database(app, &local_root, &active_le)?;

    let mut conn = Connection::open(&active_db_path)
        .map_err(|error| AppError::business_db_unavailable(&active_le).with_detail(error))?;

    // 之前出现过 WAL 里索引页写入不完整、checkpoint 把损坏带进主文件的情况，
    // 在启动时明确拦下，比让用户在业务操作里撞见 "malformed" 好。
    progress(app, 88, "正在检查数据库完整性…");
    let integrity: String = conn
        .query_row("PRAGMA integrity_check;", [], |row| row.get(0))
        .map_err(|error| AppError::business_db_unavailable(&active_le).with_detail(error))?;
    if integrity != "ok" {
        return Err(AppError::database_corrupt(&active_le, &integrity));
    }

    progress(app, 92, "正在加载中文分词扩展…");
    // 销方名称 / 备注的索引已改用 SQLite 自带的 trigram 分词，不再依赖 simple.dll / jieba 词典。
    // 这里只是遗留的加载，装不上不影响任何功能，不阻止启动。
    if let Err(error) = init_tokenizer(app, &conn) {
        warnings.push(error);
    }
    progress(app, 96, "正在初始化表结构…");
    init_db(&mut conn)
        .map_err(|error| AppError::business_db_unavailable(&active_le).with_detail(error))?;

    let runtime = AppRuntimeState {
        username: user.username,
        full_name: user.full_name,
        default_le_code: user.default_le_code,
        active_le_codes,
        active_le,
        dashboard_mode,
        is_leader: user.is_leader,
        share_available,
        admin_db_path,
        active_db_path,
        local_root,
        shared_root,
    };

    // 2'. 补备份放在拿到 runtime 之后执行，因为需要 active LE 列表。
    progress(app, 99, "正在检查待备份文件…");
    if let Some(warning) = backup::retry_pending_backups(&runtime) {
        warnings.push(warning);
    }
    progress(app, 100, "数据库就绪，正在加载主界面…");

    Ok(Initialized {
        conn,
        runtime,
        warnings,
    })
}

// ─────────────────────────────────────────────────────────────
// admin.db 读取
// ─────────────────────────────────────────────────────────────

fn detect_username() -> String {
    whoami::username().trim().to_ascii_uppercase()
}

fn load_user(admin_db_path: &Path, username: &str) -> AppResult<AdminUser> {
    let conn = open_admin_readonly(admin_db_path)?;

    let row = conn
        .query_row(
            "SELECT username, full_name, default_le_code, is_active, is_leader
             FROM app_users
             WHERE username = ?1 COLLATE NOCASE",
            [username],
            |row| {
                let is_active: i64 = row.get(3)?;
                let is_leader: i64 = row.get(4)?;
                Ok((
                    AdminUser {
                        username: row.get(0)?,
                        full_name: row.get(1)?,
                        default_le_code: row.get(2)?,
                        is_leader: is_leader == 1,
                    },
                    is_active == 1,
                ))
            },
        )
        .optional()
        .map_err(|error| AppError::admin_db_invalid().with_detail(error))?;

    match row {
        Some((user, true)) => Ok(user),
        Some((_, false)) => Err(AppError::user_disabled(username)),
        None => Err(AppError::user_not_configured(username)),
    }
}

fn load_active_le_codes(admin_db_path: &Path) -> AppResult<Vec<String>> {
    let conn = open_admin_readonly(admin_db_path)?;

    let mut statement = conn
        .prepare(
            "SELECT le_code
             FROM legal_entities
             WHERE is_active = 1
             ORDER BY le_code",
        )
        .map_err(|error| AppError::admin_db_invalid().with_detail(error))?;

    let rows = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|error| AppError::admin_db_invalid().with_detail(error))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| AppError::admin_db_invalid().with_detail(error))
}

fn open_admin_readonly(path: &Path) -> AppResult<Connection> {
    Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|error| AppError::admin_db_invalid().with_detail(error))
}

// ─────────────────────────────────────────────────────────────
// LE 数据库选择
// ─────────────────────────────────────────────────────────────

/// 选出某个 LE 当前应该使用的数据库文件。
///
/// 规则：
///   - 本地一个都没有 → 建 {le}_{当年}.db
///   - 否则用年份最大的那个
///   - 只在一月、且最新库是往年的、且已超过 800 MB 时才轮换到新年度库
///     （同一年内即使超限也继续用同一个库，避免年中数据被拆开）
pub fn ensure_le_database(app: &AppHandle, local_root: &Path, le: &str) -> AppResult<PathBuf> {
    let year = Local::now().year();
    let databases = list_le_databases(local_root, le)
        .map_err(|error| AppError::business_db_unavailable(le).with_detail(error))?;

    let Some((latest_path, latest_year)) = databases.last().cloned() else {
        let new_path = local_root.join(format!("{le}_{year}.db"));
        create_database_file(app, &new_path, le)?;
        return Ok(new_path);
    };

    let is_january = Local::now().month() == 1;
    let is_previous_year = latest_year < year;
    let is_too_large = std::fs::metadata(&latest_path)
        .map(|metadata| metadata.len() >= DB_SIZE_LIMIT_BYTES)
        .unwrap_or(false);

    if is_january && is_previous_year && is_too_large {
        let new_path = local_root.join(format!("{le}_{year}.db"));
        if !new_path.exists() {
            create_database_file(app, &new_path, le)?;
            return Ok(new_path);
        }
    }

    Ok(latest_path)
}

fn create_database_file(app: &AppHandle, path: &Path, le: &str) -> AppResult<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| AppError::business_db_unavailable(le).with_detail(error))?;
    }

    let mut conn = Connection::open(path)
        .map_err(|error| AppError::business_db_unavailable(le).with_detail(error))?;
    let _ = init_tokenizer(app, &conn);
    init_db(&mut conn)
        .map_err(|error| AppError::business_db_unavailable(le).with_detail(error))?;
    checkpoint(&conn)
        .map_err(|error| AppError::business_db_unavailable(le).with_detail(error))?;
    Ok(())
}

// ─────────────────────────────────────────────────────────────
// 中文分词扩展
// 原来这里全是 unwrap，一旦 resources 缺文件就直接闪退。
// 现在改成返回 AppError，作为 Warning 处理。
// ─────────────────────────────────────────────────────────────

pub fn init_tokenizer(app: &AppHandle, conn: &Connection) -> AppResult<()> {
    let resource_dir = app
        .path()
        .resource_dir()
        .map_err(|error| AppError::internal(error))?
        .join("resources");

    let lib_path = resource_dir.join("simple.dll");
    let lib_path_text = lib_path
        .to_str()
        .ok_or_else(|| AppError::internal("分词扩展路径包含无法处理的字符"))?;

    unsafe {
        conn.load_extension_enable().map_err(AppError::internal)?;
        conn.load_extension(lib_path_text, None::<&str>)
            .map_err(AppError::internal)?;
    }

    let dict_path = resource_dir.join("dict");
    let dict_path_text = dict_path
        .to_str()
        .ok_or_else(|| AppError::internal("分词字典路径包含无法处理的字符"))?;

    conn.query_row("SELECT jieba_dict(?1)", [dict_path_text], |_| Ok(()))
        .map_err(AppError::internal)?;

    Ok(())
}
