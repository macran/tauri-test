// src-tauri/src/state.rs
//
// 运行时状态与启动状态。
//
// 前端只通过 get_startup_status 读取三态：starting / ready / failed。
// 这样启动失败时窗口仍然能显示，不会直接崩掉。

use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicU8, Ordering},
        Arc, Mutex,
    },
};

use rusqlite::Connection;

use crate::errors::AppError;

// ─────────────────────────────────────────────────────────────
// DbState
// 保留原有结构，现有 commands 不需要改动。
// ─────────────────────────────────────────────────────────────
pub struct DbState {
    pub conn: Arc<Mutex<Connection>>,
}

impl DbState {
    pub fn new(conn: Connection) -> Self {
        Self {
            conn: Arc::new(Mutex::new(conn)),
        }
    }
}

/// 当前正在使用的 LE 与对应的本地库文件。启动时为用户的默认 LE，运行中可以被 switch_le 改掉。
/// 单独放一个可变状态，而不是改 AppRuntimeState（它被克隆到后台线程等处，且其余字段运行期不会变）。
#[derive(Clone, Debug)]
pub struct CurrentLeInner {
    pub le: String,
    pub db_path: PathBuf,
}

/// Clone 只是克隆里面的 Arc，所以可以直接移进阻塞线程；所有克隆看到的是同一份“当前 LE”。
#[derive(Clone)]
pub struct CurrentLe(Arc<Mutex<CurrentLeInner>>);

impl CurrentLe {
    pub fn new(le: String, db_path: PathBuf) -> Self {
        Self(Arc::new(Mutex::new(CurrentLeInner { le, db_path })))
    }

    pub fn get(&self) -> CurrentLeInner {
        self.0.lock().map(|g| g.clone()).unwrap_or_else(|e| e.into_inner().clone())
    }

    pub fn set(&self, le: String, db_path: PathBuf) {
        let mut guard = self.0.lock().unwrap_or_else(|e| e.into_inner());
        guard.le = le;
        guard.db_path = db_path;
    }
}

/// admin.db 中读到的当前用户。
#[derive(Clone, Debug)]
pub struct AdminUser {
    pub username: String,
    pub full_name: String,
    /// NULL 表示该用户看全部 active LE 的汇总 dashboard。
    pub default_le_code: Option<String>,
    pub is_leader: bool,
}

/// 启动成功后的运行时上下文。
#[derive(Clone, Debug, serde::Serialize, specta::Type)]
pub struct AppRuntimeState {
    pub username: String,
    pub full_name: String,
    pub default_le_code: Option<String>,
    pub active_le_codes: Vec<String>,
    pub active_le: String,
    /// default_le_code 为空时为 true，前端需要汇总所有 active LE。
    pub dashboard_mode: bool,
    pub is_leader: bool,
    /// 共享盘本次是否可用。false 时前端显示离线提示条。
    pub share_available: bool,
    #[specta(type = String)]
    pub admin_db_path: PathBuf,
    #[specta(type = String)]
    pub active_db_path: PathBuf,
    #[specta(type = String)]
    pub local_root: PathBuf,
    #[specta(type = String)]
    pub shared_root: PathBuf,
}

/// 启动进度。percent 0-100 单调递增；同时作为 `startup-progress` 事件的负载。
#[derive(Clone, Debug, serde::Serialize, specta::Type)]
pub struct StartupProgress {
    pub percent: u8,
    pub message: String,
}

/// 前端可见的启动阶段。
#[derive(Clone, Debug, serde::Serialize, specta::Type)]
#[serde(tag = "phase", rename_all = "snake_case")]
pub enum StartupStatus {
    /// 初始化进行中，前端显示启动画面。
    /// progress 为最近一次进度，前端晚于事件挂载时靠它补上。
    Starting { progress: Option<StartupProgress> },
    /// 初始化完成。warnings 为可继续使用的提示，例如共享盘不可用。
    Ready {
        runtime: AppRuntimeState,
        warnings: Vec<AppError>,
    },
    /// 初始化失败，前端显示错误页并只提供“退出”和“复制详细信息”。
    Failed { error: AppError },
}

/// 启动状态容器。setup 里先放入 Starting，初始化线程完成后写入结果。
pub struct StartupState {
    inner: Mutex<StartupStatus>,
    /// 位 1 = 初始化成功，位 2 = 前端已渲染；两位齐了才显示主窗口。
    gate: AtomicU8,
    /// main 窗口比后端初始化先到的里程碑。后端没完成前不展示（否则进度条会越过后端阶段直接跳到 70），
    /// 后端完成的那一刻再补发。
    pending_main_progress: Mutex<Option<StartupProgress>>,
    /// 最近一次进度。独立于 StartupStatus：初始化完成后状态变成 Ready，但 main 窗口加载阶段
    /// 仍在上报进度，晚挂载的 splash 要靠它补上。
    last_progress: Mutex<Option<StartupProgress>>,
    /// LocalNewer 失败时待上传的 (本地文件, 共享盘目标)。
    pending_upload: Mutex<Vec<(PathBuf, PathBuf)>>,
}

pub const GATE_INIT_OK: u8 = 1;
pub const GATE_FRONTEND_READY: u8 = 2;

impl StartupState {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(StartupStatus::Starting { progress: None }),
            gate: AtomicU8::new(0),
            last_progress: Mutex::new(None),
            pending_main_progress: Mutex::new(None),
            pending_upload: Mutex::new(Vec::new()),
        }
    }

    pub fn get(&self) -> StartupStatus {
        match self.inner.lock() {
            Ok(guard) => guard.clone(),
            // 锁被污染说明初始化线程 panic 了，按失败处理而不是继续 panic。
            Err(_) => StartupStatus::Failed {
                error: AppError::internal("启动状态锁已损坏"),
            },
        }
    }

    /// 记录进度。百分比只增不减：后端初始化与 main 窗口加载两路上报，晚到的旧值不能让进度条倒退。
    pub fn set_progress(&self, progress: StartupProgress) -> bool {
        let Ok(mut last) = self.last_progress.lock() else { return false };
        if last.as_ref().is_some_and(|p| p.percent > progress.percent) {
            return false;
        }
        *last = Some(progress.clone());
        drop(last);
        if let Ok(mut guard) = self.inner.lock() {
            if matches!(*guard, StartupStatus::Starting { .. }) {
                *guard = StartupStatus::Starting { progress: Some(progress) };
            }
        }
        true
    }

    pub fn defer_main_progress(&self, progress: StartupProgress) {
        if let Ok(mut p) = self.pending_main_progress.lock() {
            *p = Some(progress);
        }
    }

    pub fn take_main_progress(&self) -> Option<StartupProgress> {
        self.pending_main_progress.lock().ok().and_then(|mut p| p.take())
    }

    pub fn is_init_done(&self) -> bool {
        self.gate.load(Ordering::SeqCst) & GATE_INIT_OK != 0
    }

    pub fn last_progress(&self) -> Option<StartupProgress> {
        self.last_progress.lock().ok().and_then(|p| p.clone())
    }

    /// 记录一个条件已满足；仅当这次调用让两个条件第一次同时成立时返回 true。
    pub fn arrive(&self, bit: u8) -> bool {
        let previous = self.gate.fetch_or(bit, Ordering::SeqCst);
        previous | bit == GATE_INIT_OK | GATE_FRONTEND_READY
            && previous != GATE_INIT_OK | GATE_FRONTEND_READY
    }

    pub fn is_finished(&self) -> bool {
        self.gate.load(Ordering::SeqCst) == GATE_INIT_OK | GATE_FRONTEND_READY
    }

    pub fn set_pending_upload(&self, files: Vec<(PathBuf, PathBuf)>) {
        if let Ok(mut guard) = self.pending_upload.lock() {
            *guard = files;
        }
    }

    pub fn take_pending_upload(&self) -> Vec<(PathBuf, PathBuf)> {
        self.pending_upload
            .lock()
            .map(|mut guard| std::mem::take(&mut *guard))
            .unwrap_or_default()
    }

    pub fn set(&self, status: StartupStatus) {
        if let Ok(mut guard) = self.inner.lock() {
            *guard = status;
        }
    }
}

impl Default for StartupState {
    fn default() -> Self {
        Self::new()
    }
}
