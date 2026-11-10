// src-tauri/src/errors.rs
//
// 统一的错误类型。
//
// 关键设计：错误分成两类。
//   Fatal   ：软件无法进入业务界面，前端显示启动错误页。
//   Warning ：软件可以继续运行，前端显示提示条。
//
// 所有面向用户的文案都放在这里，避免把 SQLite 或 io 的英文错误直接抛给用户。

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum ErrorSeverity {
    /// 无法启动，必须由管理员或 IT 处理。
    Fatal,
    /// 可以继续使用，但功能受限。
    Warning,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    // ── Fatal ────────────────────────────────────────────────
    /// 拿不到 %LOCALAPPDATA%。
    LocalDirUnavailable,
    /// 共享盘和本地缓存都没有 admin.db。
    AdminDbMissing,
    /// admin.db 打不开或缺少必需的表。
    AdminDbInvalid,
    /// admin.db 里没有当前 Windows 用户。
    UserNotConfigured,
    /// 用户存在但 is_active = 0。
    UserDisabled,
    /// legal_entities 里没有 is_active = 1 的记录。
    NoActiveLegalEntity,
    /// 用户的 default_le_code 不在 active LE 列表中。
    DefaultLegalEntityInvalid,
    /// 业务数据库无法打开或建表失败。
    BusinessDbUnavailable,
    /// 本地数据库比共享盘新（上次可能未正常退出，改动未上传）。detail 为换行分隔的文件名。
    LocalNewer,
    /// 本地数据库被其他程序占用，无法用共享盘版本替换。
    LocalDbLocked,
    /// integrity_check 未通过。
    DatabaseCorrupt,

    // ── Warning ──────────────────────────────────────────────
    /// 共享盘当前不可访问。
    ShareUnavailable,
    /// 某个 LE 的数据库同步失败。
    SyncFailed,
    /// 本地有改动但还没有备份到共享盘。
    BackupPending,
    /// 备份过程本身出错。
    BackupFailed,

    /// 兜底。
    Internal,
}

impl ErrorCode {
    pub fn severity(self) -> ErrorSeverity {
        match self {
            ErrorCode::LocalDirUnavailable
            | ErrorCode::AdminDbMissing
            | ErrorCode::AdminDbInvalid
            | ErrorCode::UserNotConfigured
            | ErrorCode::UserDisabled
            | ErrorCode::NoActiveLegalEntity
            | ErrorCode::DefaultLegalEntityInvalid
            | ErrorCode::BusinessDbUnavailable
            | ErrorCode::LocalNewer
            | ErrorCode::LocalDbLocked
            | ErrorCode::DatabaseCorrupt
            | ErrorCode::Internal => ErrorSeverity::Fatal,

            ErrorCode::ShareUnavailable
            | ErrorCode::SyncFailed
            | ErrorCode::BackupPending
            | ErrorCode::BackupFailed => ErrorSeverity::Warning,
        }
    }
}

/// 面向用户的错误。
///
/// `message` 是给最终用户看的中文说明。
/// `detail` 是原始技术错误，只在“显示详细信息”里展示，方便你排查。
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct AppError {
    pub code: ErrorCode,
    pub severity: ErrorSeverity,
    pub message: String,
    pub detail: Option<String>,
}

impl AppError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            severity: code.severity(),
            message: message.into(),
            detail: None,
        }
    }

    pub fn with_detail(mut self, detail: impl fmt::Display) -> Self {
        self.detail = Some(detail.to_string());
        self
    }

    pub fn is_fatal(&self) -> bool {
        self.severity == ErrorSeverity::Fatal
    }

    // ── 常用错误的构造函数，集中管理用户文案 ──────────────────

    pub fn local_dir_unavailable() -> Self {
        Self::new(
            ErrorCode::LocalDirUnavailable,
            "无法访问本机的应用数据目录，请联系 IT 支持。",
        )
    }

    pub fn admin_db_missing(shared_root: &str) -> Self {
        Self::new(
            ErrorCode::AdminDbMissing,
            format!(
                "找不到用户配置数据库 admin.db。\n\n\
                 请确认可以访问共享盘：\n{shared_root}\n\n\
                 如果网络正常，请联系 Input Tax Leader 确认 admin.db 是否已放入该目录。"
            ),
        )
    }

    pub fn admin_db_invalid() -> Self {
        Self::new(
            ErrorCode::AdminDbInvalid,
            "用户配置数据库 admin.db 无法读取，可能已损坏。\n\n请联系 Input Tax Leader。",
        )
    }

    pub fn user_not_configured(username: &str) -> Self {
        Self::new(
            ErrorCode::UserNotConfigured,
            format!(
                "当前 Windows 用户 {username} 尚未配置使用权限。\n\n\
                 请联系 Input Tax Leader 在用户管理中添加该账号。"
            ),
        )
    }

    pub fn user_disabled(username: &str) -> Self {
        Self::new(
            ErrorCode::UserDisabled,
            format!(
                "当前 Windows 用户 {username} 已停用。\n\n如需恢复使用，请联系 Input Tax Leader。"
            ),
        )
    }

    pub fn no_active_legal_entity() -> Self {
        Self::new(
            ErrorCode::NoActiveLegalEntity,
            "用户配置中没有启用任何 LE。\n\n请联系 Input Tax Leader 检查 LE 设置。",
        )
    }

    pub fn default_legal_entity_invalid(username: &str, le_code: &str) -> Self {
        Self::new(
            ErrorCode::DefaultLegalEntityInvalid,
            format!(
                "用户 {username} 的默认 LE {le_code} 不存在或已停用。\n\n请联系 Input Tax Leader 修改默认 LE。"
            ),
        )
    }

    pub fn business_db_unavailable(le_code: &str) -> Self {
        Self::new(
            ErrorCode::BusinessDbUnavailable,
            format!("LE {le_code} 的发票数据库无法打开。\n\n请重启软件，如仍失败请联系 IT 支持。"),
        )
    }

    pub fn local_newer(files: &[String]) -> Self {
        Self::new(
            ErrorCode::LocalNewer,
            format!(
                "本地数据库比共享盘新：{}。

可能上次未正常退出，改动尚未上传。                 如确认本地数据正确，请点击“立即上传到共享盘”。",
                files.join("、")
            ),
        )
        .with_detail(files.join("
"))
    }

    pub fn local_db_locked(file_name: &str) -> Self {
        Self::new(
            ErrorCode::LocalDbLocked,
            format!(
                "本地数据库 {file_name} 被其他程序占用，无法更新。

                 请先关闭占用它的程序（如数据库工具、另一个本软件窗口），再重启软件。"
            ),
        )
    }

    pub fn database_corrupt(le_code: &str, result: &str) -> Self {
        Self::new(
            ErrorCode::DatabaseCorrupt,
            format!(
                "LE {le_code} 的数据库文件已损坏。

请勿继续写入，联系管理员用                  `sqlite3 <db> \".recover\"` 恢复数据后再启动。"
            ),
        )
        .with_detail(format!("integrity_check: {result}"))
    }

    pub fn share_unavailable(shared_root: &str) -> Self {
        Self::new(
            ErrorCode::ShareUnavailable,
            format!(
                "共享盘当前无法访问，本次使用本机数据。\n\n路径：{shared_root}\n\n\
                 你的修改会保存在本机，恢复网络后软件会自动备份。"
            ),
        )
    }

    pub fn sync_failed(le_code: &str) -> Self {
        Self::new(
            ErrorCode::SyncFailed,
            format!("LE {le_code} 的数据同步未完成，本次使用本机数据。"),
        )
    }

    pub fn backup_pending(files: &[String]) -> Self {
        Self::new(
            ErrorCode::BackupPending,
            format!(
                "有 {} 个数据库还没有备份到共享盘。\n\n恢复网络后软件会自动备份，也可以点击“立即备份”。",
                files.len()
            ),
        )
    }

    pub fn backup_failed(detail: impl fmt::Display) -> Self {
        Self::new(ErrorCode::BackupFailed, "备份到共享盘时出错，数据已保留在本机。")
            .with_detail(detail)
    }

    pub fn internal(detail: impl fmt::Display) -> Self {
        Self::new(ErrorCode::Internal, "软件启动时发生未预期的错误。").with_detail(detail)
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.detail {
            Some(detail) => write!(formatter, "[{:?}] {} ({detail})", self.code, self.message),
            None => write!(formatter, "[{:?}] {}", self.code, self.message),
        }
    }
}

impl std::error::Error for AppError {}

impl From<std::io::Error> for AppError {
    fn from(error: std::io::Error) -> Self {
        AppError::internal(error)
    }
}

impl From<rusqlite::Error> for AppError {
    fn from(error: rusqlite::Error) -> Self {
        AppError::internal(error)
    }
}

impl From<serde_json::Error> for AppError {
    fn from(error: serde_json::Error) -> Self {
        AppError::internal(error)
    }
}

pub type AppResult<T> = Result<T, AppError>;
