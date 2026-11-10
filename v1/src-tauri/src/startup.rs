// src-tauri/src/startup.rs
//
// 启动阶段的数据库同步与状态上报。
// 任何同步失败都不再让 setup 直接报错退出，而是记录到 StartupState，
// 由前端启动页展示；失败时不注册 DbState / AppRuntimeState，
// 因此退出时 backup_on_exit 不会用旧库覆盖共享盘。

use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, SystemTime},
};

use rusqlite::{Connection, OpenFlags};
use serde::Serialize;
use specta::Type;
use tauri::{AppHandle, Emitter, Manager};

use crate::{
    checkpoint, current_year, detect_username, ensure_le_database, init_db, init_tokenizer,
    list_le_databases, load_admin_context, spawn_periodic_checkpoint, validate_admin_database,
    verify_database_integrity, AppRuntimeState, DbState, ADMIN_DB_FILE, LOCAL_DB_DIR, SHARED_ROOT,
};

/// 把当前启动步骤推给 splash 窗口显示。
fn step(app: &AppHandle, text: &str) {
    let _ = app.emit("startup-step", text);
}

/// 修改时间相差在此范围内视为相同（共享盘文件系统时间精度可能较粗）。
const MTIME_TOLERANCE: Duration = Duration::from_secs(2);

// ─────────────────────────────────────────────
// 对前端暴露的状态
// ─────────────────────────────────────────────
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum StartupFailureKind {
    /// 本地数据库被其他程序占用，无法用共享盘版本替换。
    LocalLocked,
    /// 无法访问共享盘，或从共享盘复制中断。
    SharedUnreachable,
    /// 本地比共享盘新，说明有尚未上传的改动。
    LocalNewer,
    Other,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct StartupStatus {
    pub ready: bool,
    pub kind: Option<StartupFailureKind>,
    pub message: String,
    /// 本地更新、待上传的文件名（仅 LocalNewer 时非空）。
    pub files: Vec<String>,
}

impl StartupStatus {
    fn ready() -> Self {
        Self { ready: true, kind: None, message: String::new(), files: vec![] }
    }
}

pub(crate) struct StartupFailure {
    kind: StartupFailureKind,
    message: String,
    /// (本地文件, 共享盘目标文件)，供“立即上传”使用。
    upload: Vec<(PathBuf, PathBuf)>,
}

impl StartupFailure {
    fn new(kind: StartupFailureKind, message: impl Into<String>) -> Self {
        Self { kind, message: message.into(), upload: vec![] }
    }

    pub(crate) fn other(error: impl std::fmt::Display) -> Self {
        Self::new(StartupFailureKind::Other, error.to_string())
    }

    fn status(&self) -> StartupStatus {
        StartupStatus {
            ready: false,
            kind: Some(self.kind),
            message: self.message.clone(),
            files: self
                .upload
                .iter()
                .filter_map(|(local, _)| local.file_name()?.to_str().map(str::to_owned))
                .collect(),
        }
    }
}

struct StartupInner {
    status: StartupStatus,
    pending_upload: Vec<(PathBuf, PathBuf)>,
}

pub struct StartupState(Mutex<StartupInner>);

impl StartupState {
    pub fn new() -> Self {
        Self(Mutex::new(StartupInner {
            status: StartupStatus {
                ready: false,
                kind: None,
                message: "正在启动…".into(),
                files: vec![],
            },
            pending_upload: vec![],
        }))
    }
}

// ─────────────────────────────────────────────
// 文件比较与复制
// ─────────────────────────────────────────────
enum Decision {
    Skip,
    Pull,
    LocalNewer,
}

fn mtime_close(a: SystemTime, b: SystemTime) -> bool {
    let diff = a.duration_since(b).unwrap_or_else(|e| e.duration());
    diff <= MTIME_TOLERANCE
}

/// 比较共享盘文件与本地文件。remote 的读取错误原样返回，由调用方判定为共享盘不可用。
fn decide(remote: &Path, local: &Path) -> io::Result<Decision> {
    let remote_meta = fs::metadata(remote)?;
    let local_meta = match fs::metadata(local) {
        Ok(meta) => meta,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Decision::Pull),
        Err(error) => return Err(error),
    };
    let (remote_time, local_time) = (remote_meta.modified()?, local_meta.modified()?);

    Ok(if mtime_close(remote_time, local_time) {
        // 时间相同而大小不同属于异常，以共享盘为准。
        if remote_meta.len() == local_meta.len() { Decision::Skip } else { Decision::Pull }
    } else if remote_time > local_time {
        Decision::Pull
    } else {
        Decision::LocalNewer
    })
}

fn is_locked(error: &io::Error) -> bool {
    // 5 拒绝访问 / 32 共享冲突 / 33 锁定冲突
    error.kind() == io::ErrorKind::PermissionDenied
        || matches!(error.raw_os_error(), Some(5 | 32 | 33))
}

fn file_name(path: &Path) -> String {
    path.file_name().and_then(|v| v.to_str()).unwrap_or_default().to_owned()
}

/// 先复制到本地临时文件；任何失败都会清理临时文件。
fn copy_to_incoming(source: &Path, destination: &Path) -> Result<PathBuf, StartupFailure> {
    let name = file_name(destination);
    let incoming = destination.with_file_name(format!(".{name}.incoming-{}", std::process::id()));
    fs::copy(source, &incoming).map_err(|error| {
        let _ = fs::remove_file(&incoming);
        StartupFailure::new(
            StartupFailureKind::SharedUnreachable,
            format!("从共享盘复制 {name} 失败，请检查网络后重试；如仍失败，请联系 IT 协助访问共享网盘。\n{error}"),
        )
    })?;
    Ok(incoming)
}

/// 用临时文件替换本地文件。rename 在 Windows 上会覆盖目标，失败时本地原文件保持不变。
fn install_incoming(incoming: &Path, destination: &Path) -> Result<(), StartupFailure> {
    if let Err(error) = fs::rename(incoming, destination) {
        let _ = fs::remove_file(incoming);
        let name = file_name(destination);
        return Err(if is_locked(&error) {
            StartupFailure::new(
                StartupFailureKind::LocalLocked,
                format!("本地数据库 {name} 被其他程序占用，无法更新。请先关闭占用它的程序（如终端、数据库工具），再点击“重试”。"),
            )
        } else {
            StartupFailure::other(format!("替换本地数据库 {name} 失败：{error}"))
        });
    }

    // 旧库遗留的 WAL/SHM 若被套用到新文件上会导致损坏，替换成功后清掉。
    for suffix in ["-wal", "-shm"] {
        let _ = fs::remove_file(destination.with_file_name(format!("{}{suffix}", file_name(destination))));
    }
    Ok(())
}

fn shared_unreachable(error: impl std::fmt::Display) -> StartupFailure {
    StartupFailure::new(
        StartupFailureKind::SharedUnreachable,
        format!("无法连接共享盘，数据库更新失败。请检查网络或 VPN 后重试；如仍无法访问，请联系 IT 协助开通共享网盘访问权限。\n{error}"),
    )
}

/// 清理历史失败遗留的 `.xxx.incoming-<pid>` 临时文件；正在被使用的文件删除会失败，直接忽略。
fn remove_stale_incoming(local_root: &Path) {
    let Ok(entries) = fs::read_dir(local_root) else { return };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if name.starts_with('.') && name.contains(".incoming-") {
            let _ = fs::remove_file(entry.path());
        }
    }
}

// ─────────────────────────────────────────────
// 同步
// ─────────────────────────────────────────────

/// admin.db 只读缓存：与共享盘不一致就以共享盘为准。
fn sync_admin_database(local_root: &Path, shared_root: &Path) -> Result<PathBuf, StartupFailure> {
    fs::metadata(shared_root).map_err(shared_unreachable)?;
    let remote_path = shared_root.join(ADMIN_DB_FILE);
    let local_path = local_root.join(ADMIN_DB_FILE);

    let decision = match decide(&remote_path, &local_path) {
        Ok(decision) => decision,
        Err(error) if error.kind() == io::ErrorKind::NotFound && local_path.exists() => {
            eprintln!("共享盘上没有 {ADMIN_DB_FILE}，使用本地缓存");
            Decision::Skip
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Err(StartupFailure::other(format!("找不到共享盘或本地缓存中的 {ADMIN_DB_FILE}")));
        }
        Err(error) => return Err(shared_unreachable(error)),
    };

    if !matches!(decision, Decision::Skip) {
        let incoming = copy_to_incoming(&remote_path, &local_path)?;
        match validate_admin_database(&incoming) {
            Ok(()) => {
                install_incoming(&incoming, &local_path)?;
                return Ok(local_path);
            }
            Err(error) => {
                let _ = fs::remove_file(&incoming);
                if !local_path.exists() {
                    return Err(StartupFailure::other(error));
                }
                eprintln!("共享盘 admin.db 校验失败，使用本地缓存：{error}");
            }
        }
    }

    validate_admin_database(&local_path).map_err(StartupFailure::other)?;
    Ok(local_path)
}

/// 拉取某个 LE 在共享盘上的数据库。返回“本地比共享盘新”的文件，这些文件不会被覆盖。
fn sync_le_databases(
    local_root: &Path,
    shared_root: &Path,
    le: &str,
) -> Result<Vec<(PathBuf, PathBuf)>, StartupFailure> {
    fs::metadata(shared_root).map_err(shared_unreachable)?;

    let remote_dir = shared_root.join(le);
    let entries = match fs::read_dir(&remote_dir) {
        Ok(entries) => entries,
        // 共享盘可达但还没有该 LE 的目录：首次使用。
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(vec![]),
        Err(error) => return Err(shared_unreachable(error)),
    };

    let prefix = format!("{le}_");
    let mut local_newer = Vec::new();
    for entry in entries {
        let source = entry.map_err(shared_unreachable)?.path();
        if !source.is_file() || source.extension().and_then(|v| v.to_str()) != Some("db") {
            continue;
        }
        if !file_name(&source).starts_with(&prefix) {
            continue;
        }

        let destination = local_root.join(file_name(&source));
        match decide(&source, &destination).map_err(shared_unreachable)? {
            Decision::Skip => {}
            Decision::Pull => {
                let incoming = copy_to_incoming(&source, &destination)?;
                install_incoming(&incoming, &destination)?;
            }
            Decision::LocalNewer => local_newer.push((destination, source)),
        }
    }
    Ok(local_newer)
}

/// 当前 LE 必须与共享盘一致才能进入；本地有未上传改动时交给用户决定。
fn sync_active_le(local_root: &Path, shared_root: &Path, le: &str) -> Result<(), StartupFailure> {
    let local_newer = sync_le_databases(local_root, shared_root, le)?;
    if local_newer.is_empty() {
        return Ok(());
    }
    let names: Vec<String> = local_newer.iter().map(|(local, _)| file_name(local)).collect();
    let mut failure = StartupFailure::new(
        StartupFailureKind::LocalNewer,
        format!(
            "本地数据库比共享盘新：{}。可能上次未正常退出，改动尚未上传。\
             如确认本地数据正确，请点击“立即上传到共享盘”。",
            names.join("、")
        ),
    );
    failure.upload = local_newer;
    Err(failure)
}

// ─────────────────────────────────────────────
// 启动流程
// ─────────────────────────────────────────────
fn try_bootstrap(app: &AppHandle) -> Result<(), StartupFailure> {
    let username = detect_username();
    step(app, "正在检测共享盘连接…");

    let local_root = dirs::data_local_dir()
        .ok_or_else(|| StartupFailure::other("无法获取用户数据目录"))?
        .join(LOCAL_DB_DIR);
    fs::create_dir_all(&local_root).map_err(StartupFailure::other)?;
    remove_stale_incoming(&local_root);

    let shared_root = PathBuf::from(SHARED_ROOT);
    step(app, "正在同步管理配置…");
    let admin_db_path = sync_admin_database(&local_root, &shared_root)?;
    let (user, active_le_codes) =
        load_admin_context(&admin_db_path, &username).map_err(StartupFailure::other)?;

    if active_le_codes.is_empty() {
        return Err(StartupFailure::other("admin.db 中没有 active 的 LE"));
    }

    // default_le_code 为 NULL 表示全 LE dashboard。
    // 当前 DbState 仍保持单连接以兼容现有 commands；dashboard_mode 表明前端需要汇总 active LE。
    let dashboard_mode = user.default_le_code.is_none();
    let active_le = user
        .default_le_code
        .clone()
        .unwrap_or_else(|| active_le_codes[0].clone());

    step(app, "正在检查数据库更新…");
    // 其他 LE 只尽力同步，失败不拦启动；切换到该 LE 时会再检查。
    for le in active_le_codes.iter().filter(|le| **le != active_le) {
        if let Err(error) = sync_le_databases(&local_root, &shared_root, le) {
            eprintln!("同步 LE {le} 失败，将继续使用已有本地文件：{}", error.message);
        }
    }
    sync_active_le(&local_root, &shared_root, &active_le)?;

    // 本地和共享盘都没有该 LE 的数据库时，ensure_le_database 会新建。
    let first_run = list_le_databases(&local_root, &active_le)
        .map(|dbs| dbs.is_empty())
        .unwrap_or(false);
    step(app, if first_run { "正在初始化数据库…" } else { "正在连接数据库…" });
    let active_db_path = ensure_le_database(app, &local_root, &active_le, current_year())
        .map_err(StartupFailure::other)?;

    let mut conn = Connection::open(&active_db_path).map_err(StartupFailure::other)?;
    init_tokenizer(app, &conn);

    // 打开后立即体检：损坏的库不应该带着一堆奇怪的下游报错悄悄跑起来。
    step(app, "正在检查数据库完整性…");
    verify_database_integrity(&conn).map_err(StartupFailure::other)?;
    init_db(&mut conn).map_err(StartupFailure::other)?;

    // 上次若异常退出（强杀/崩溃），backup_on_exit 里的 checkpoint 不会执行，
    // WAL 会越攒越大；启动时这里还没有并发访问，抓紧机会做一次 TRUNCATE。
    if let Err(error) = checkpoint(&conn) {
        eprintln!("启动时 WAL checkpoint 失败：{error}");
    }

    step(app, "正在启动程序…");
    let db_state = DbState::new(conn);
    spawn_periodic_checkpoint(Arc::clone(&db_state.conn));

    app.manage(db_state);
    app.manage(AppRuntimeState {
        username: user.username,
        full_name: user.full_name,
        default_le_code: user.default_le_code,
        active_le_codes,
        active_le,
        dashboard_mode,
        is_leader: user.is_leader,
        admin_db_path,
        active_db_path,
        local_root,
        shared_root,
    });
    Ok(())
}

/// 运行启动流程并把结果写入 StartupState。
pub(crate) fn bootstrap(app: &AppHandle) -> StartupStatus {
    let (status, pending_upload) = match try_bootstrap(app) {
        Ok(()) => (StartupStatus::ready(), vec![]),
        Err(failure) => {
            eprintln!("启动失败：{}", failure.message);
            (failure.status(), failure.upload)
        }
    };
    {
        let state = app.state::<StartupState>();
        let mut inner = state.0.lock().unwrap();
        inner.status = status.clone();
        inner.pending_upload = pending_upload;
    }
    if status.ready {
        if let Err(error) = open_main_window(app) {
            eprintln!("创建主窗口失败：{error}");
            return StartupStatus {
                ready: false,
                kind: Some(StartupFailureKind::Other),
                message: format!("创建主窗口失败：{error}"),
                files: vec![],
            };
        }
    }
    status
}

/// 主窗口在 tauri.conf.json 中 create=false，同步通过后才创建，避免前端在数据库就绪前调用命令。
fn open_main_window(app: &AppHandle) -> tauri::Result<()> {
    if app.get_webview_window("main").is_none() {
        let config = app
            .config()
            .app
            .windows
            .iter()
            .find(|w| w.label == "main")
            .expect("tauri.conf.json 缺少 main 窗口配置");
        tauri::WebviewWindowBuilder::from_config(app, config)?.build()?;
    }
    // 先建主窗口再关 splash，否则最后一个窗口关闭会导致应用退出。
    if let Some(splash) = app.get_webview_window("splash") {
        splash.close()?;
    }
    Ok(())
}

fn upload_file(local: &Path, remote: &Path) -> io::Result<()> {
    // 未 checkpoint 的 WAL 内容不在主文件里，上传前先合并。
    let conn = Connection::open_with_flags(
        local,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(io::Error::other)?;
    checkpoint(&conn).map_err(io::Error::other)?;
    drop(conn);

    let temporary = remote.with_file_name(format!(".{}.upload-{}", file_name(remote), std::process::id()));
    let result = fs::copy(local, &temporary).and_then(|_| fs::rename(&temporary, remote));
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

// ─────────────────────────────────────────────
// Tauri commands
// ─────────────────────────────────────────────
#[tauri::command]
#[specta::specta]
pub async fn retry_startup(app: AppHandle) -> Result<StartupStatus, String> {
    if app.state::<StartupState>().0.lock().unwrap().status.ready {
        return Ok(StartupStatus::ready());
    }
    tokio::task::spawn_blocking(move || bootstrap(&app))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn upload_local_to_shared(app: AppHandle) -> Result<StartupStatus, String> {
    let pending = {
        let state = app.state::<StartupState>();
        let inner = state.0.lock().unwrap();
        if inner.status.ready {
            return Ok(StartupStatus::ready());
        }
        inner.pending_upload.clone()
    };
    if pending.is_empty() {
        return Err("没有待上传的文件".into());
    }

    tokio::task::spawn_blocking(move || {
        for (local, remote) in &pending {
            if let Err(error) = upload_file(local, remote) {
                let mut failure = StartupFailure::new(
                    StartupFailureKind::LocalNewer,
                    format!("上传 {} 失败：{error}", file_name(local)),
                );
                failure.upload = pending.clone();
                let status = failure.status();
                app.state::<StartupState>().0.lock().unwrap().status = status.clone();
                return status;
            }
        }
        // 上传成功后重新走完整启动流程。
        bootstrap(&app)
    })
    .await
    .map_err(|e| e.to_string())
}
