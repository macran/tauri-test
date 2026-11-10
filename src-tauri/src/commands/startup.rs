// src-tauri/src/commands/startup.rs
//
// 前端读取启动状态与备份状态的三个命令。
// 这些命令永远返回 Ok：错误本身就是返回值的一部分，前端不用 try/catch。

use tauri::{AppHandle, Emitter, Manager};

use crate::{
    backup,
    errors::AppError,
    state::{AppRuntimeState, StartupProgress, StartupState, StartupStatus, GATE_FRONTEND_READY},
    sync::{checkpoint, copy_atomic},
};

/// 前端启动后轮询这个命令，拿到 starting / ready / failed。
#[tauri::command]
#[specta::specta]
pub fn get_startup_status(app: AppHandle) -> StartupStatus {
    match app.try_state::<StartupState>() {
        Some(state) => state.get(),
        None => StartupStatus::Starting { progress: None },
    }
}

/// 还没备份到共享盘的文件名列表。空列表表示一切正常。
#[tauri::command]
#[specta::specta]
pub fn get_pending_backups(app: AppHandle) -> Vec<String> {
    match app.try_state::<AppRuntimeState>() {
        Some(runtime) => backup::pending_file_names(&runtime.local_root),
        None => Vec::new(),
    }
}

/// 用户点击“立即备份”。返回剩余的待备份文件，空列表表示全部成功。
#[tauri::command]
#[specta::specta]
pub fn retry_backup_now(app: AppHandle) -> Vec<String> {
    let Some(runtime) = app.try_state::<AppRuntimeState>() else {
        return Vec::new();
    };

    let runtime = runtime.inner().clone();
    let _ = backup::backup_all(&runtime);
    backup::pending_file_names(&runtime.local_root)
}

/// 前端完成首次渲染（骨架屏已经画出来）后调用一次，通知 Rust 侧
/// 现在才是显示主窗口的合适时机——避免窗口在 webview 还没画出任何
/// 内容前就被显示出来，造成一段空白闪屏。
#[tauri::command]
#[specta::specta]
pub fn notify_frontend_ready(app: AppHandle) {
    eprintln!("[startup] notify_frontend_ready @ {}", chrono::Local::now().format("%H:%M:%S%.3f"));
    if let Some(state) = app.try_state::<StartupState>() {
        if state.arrive(GATE_FRONTEND_READY) {
            // 主窗口可以显示了：进度满格，随即关闭 splash
            publish_progress(&app, 100, "启动完成");
            crate::finish_startup(&app);
        }
    }
}

/// LocalNewer 失败页的“立即上传到共享盘”。成功后不关闭程序，由用户自己点「重新登录」。
#[tauri::command]
#[specta::specta]
pub fn upload_local_to_shared(app: AppHandle) -> Result<(), AppError> {
    let state = app
        .try_state::<StartupState>()
        .ok_or_else(|| AppError::internal("启动状态不可用"))?;
    let files = state.take_pending_upload();
    for (local, remote) in &files {
        // 本地库是 WAL 模式：最近的写入可能还在 -wal 文件里，只拷 .db 会丢掉它们。
        // 失败态下应用的数据库连接还没建，所以临时打开一次本地文件，把 WAL 合并进主文件再上传。
        if let Err(error) = checkpoint_local(local) {
            state.set_pending_upload(files.clone());
            return Err(AppError::backup_failed(format!("合并本地数据库的写入日志失败：{error}")));
        }
        if let Err(error) = copy_atomic(local, remote) {
            // 放回去，用户修好网络后可以再点一次。
            state.set_pending_upload(files.clone());
            return Err(AppError::backup_failed(error));
        }
    }
    Ok(())
}

/// 临时打开本地 .db，把 WAL 合并进主文件（TRUNCATE 会同时清空 -wal）。
/// 本地库被别的进程占用时这里会失败，错误会原样告诉用户，而不是悄悄只传一个不完整的 .db。
fn checkpoint_local(path: &std::path::Path) -> Result<(), String> {
    let conn = rusqlite::Connection::open(path).map_err(|e| e.to_string())?;
    conn.busy_timeout(std::time::Duration::from_secs(5)).map_err(|e| e.to_string())?;
    checkpoint(&conn).map_err(|e| e.to_string())
}

/// 重启软件（前端「重新登录」用）。
///
/// 不能直接用插件的 relaunch()：Tauri 的重启是先 spawn 新进程、再退出旧进程，这段时间里旧进程
/// 仍持有单实例互斥量，新进程会被判定为「第二个实例」而直接退出，结果软件整个消失。
/// 所以先释放互斥量，再请求重启。
#[tauri::command]
#[specta::specta]
pub fn restart_app(app: AppHandle) {
    tauri_plugin_single_instance::destroy(&app);
    app.request_restart();
}

/// 写入并广播进度（后端初始化和 main 窗口加载共用）。被丢弃的旧值（百分比倒退）不广播。
pub(crate) fn publish_progress(app: &AppHandle, percent: u8, message: &str) {
    let payload = StartupProgress { percent, message: message.to_string() };
    let accepted = app
        .try_state::<StartupState>()
        .map(|state| state.set_progress(payload.clone()))
        .unwrap_or(true);
    if accepted {
        let _ = app.emit("startup-progress", payload);
    }
}

/// main 窗口上报自己的加载里程碑，占进度轴的 60..=100。
/// 100 由 notify_frontend_ready 隐含（它到达时 splash 随即被关闭），这里不接受 100，避免提前满格。
#[tauri::command]
#[specta::specta]
pub fn report_main_progress(app: AppHandle, percent: u8, message: String) {
    let percent = percent.clamp(60, 99);
    if let Some(state) = app.try_state::<StartupState>() {
        if !state.is_init_done() {
            // 后端还在初始化：先记下，等它完成时补发，避免进度条越过后端阶段
            state.defer_main_progress(StartupProgress { percent, message });
            return;
        }
    }
    publish_progress(&app, percent, &message);
}

/// 后端初始化完成时调用：把 main 窗口先到的里程碑补发出去。
pub(crate) fn flush_main_progress(app: &AppHandle) {
    if let Some(p) = app.try_state::<StartupState>().and_then(|s| s.take_main_progress()) {
        publish_progress(app, p.percent, &p.message);
    }
}

/// splash 晚于事件挂载时，用它补上最近一次进度（含 main 窗口加载阶段）。
#[tauri::command]
#[specta::specta]
pub fn get_startup_progress(app: AppHandle) -> Option<StartupProgress> {
    app.try_state::<StartupState>().and_then(|s| s.last_progress())
}
