// src-tauri/src/error.rs

// use strum::Display;
use thiserror::Error;

// ─────────────────────────────────────────────────────────────
// AppError
//
// 分为三类：
//   1. 基础设施错误：文件、数据库、IO
//   2. Excel 解析错误：全部带 sheet + row + col，由 parser 层构造
//   3. 业务错误：无法生成唯一键等
//
// 辅助函数（raw_row.rs）只返回 CellError（轻量原因）。
// parser 层调用时用 .map_err() 补充 sheet/row/col 构造完整 AppError。
// ─────────────────────────────────────────────────────────────
#[derive(Debug, Error)]
pub enum AppError {
    // ────────────────────
    // 1. 基础设施错误
    // ────────────────────
    #[error("数据库错误: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("Excel 解析错误: {0}")]
    ExcelParse(String),
    // 如果返回的是 XlsxError
    #[error("Excel 文件错误: {0}")]
    Excel(#[from] calamine::XlsxError),

    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),

    #[error("Excel 中找不到 Sheet: \"{0}\"")]
    SheetNotFound(String),

    // ────────────────────
    // 2. Excel 解析错误
    //    全部带 sheet + row + col
    //    由 parser 层调用 .map_err() 构造
    // ────────────────────
    #[error("[{sheet}] 第 {row} 行「{col}」不能为空")]
    MissingField {
        sheet: &'static str,
        row: u32,
        col: &'static str,
    },

    #[error("[{sheet}] 第 {row} 行「{col}」枚举解析失败，未知字符串: \"{value}\"")]
    EnumParse {
        sheet: &'static str,
        row: u32,
        col: &'static str,
        value: String,
    },

    #[error("[{sheet}] 第 {row} 行「{col}」金额格式错误，内容: \"{value}\"")]
    AmountParse {
        sheet: &'static str,
        row: u32,
        col: &'static str,
        value: String,
    },

    // ────────────────────
    // 3. 业务错误
    // ────────────────────
    #[error("[{sheet}] 第 {row} 行发票代码/发票号码/数电发票号码均为空，无法生成唯一键")]
    MissingUnqKey { sheet: &'static str, row: u32 },
    // 在 error.rs 中添加：
    #[error("线程 panic: {0}")]
    ThreadPanic(String),

    #[error("其他错误: {0}")]
    Unknown(String),
}

// Tauri command 要求 AppError 实现 serde::Serialize
impl serde::Serialize for AppError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

// ─────────────────────────────────────────────────────────────
// CellError
//
// raw_row.rs 辅助函数返回的轻量错误原因。
// 不带任何上下文信息（无 sheet/row/col）。
// parser 层调用时用 .map_err(|_| AppError::XxxField { ... }) 补充上下文。
// ─────────────────────────────────────────────────────────────
#[derive(Debug)]
pub enum CellError {
    /// 单元格为空（Empty 或空白字符串）
    Empty,
    /// 单元格内容无法转换为数字
    InvalidNumber,
}

// ─────────────────────────────────────────────────────────────
// RowParseError
//
// parser / operations 层收集的单行错误。
// 和 AppError 的区别：
//   - AppError 是致命错误，中断整个流程
//   - RowParseError 是可收集错误，记录单行问题后继续解析下一行
// 同时用于前端展示错误单元格信息
// ─────────────────────────────────────────────────────────────
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RowParseError {
    /// Excel 行号（从 2 开始，第 1 行为表头）
    pub sheet: String,
    pub row: u32,
    /// 出错的列名（中文列名）
    pub col: String,
    /// 错误描述
    pub message: String,
}

impl RowParseError {
    pub fn new(row: u32, col: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            sheet: "".to_string(),
            row,
            col: col.into(),
            message: message.into(),
        }
    }

    /// 从 AppError 中提取 row/col/message 构建 RowParseError。
    /// 只适用于 Excel 解析类 AppError 变体。
    pub fn from_app_error(err: &AppError) -> Option<Self> {
        match err {
            AppError::MissingField { row, col, .. } => Some(Self::new(*row, *col, err.to_string())),
            AppError::EnumParse { row, col, .. } => Some(Self::new(*row, *col, err.to_string())),
            AppError::AmountParse { row, col, .. } => Some(Self::new(*row, *col, err.to_string())),
            AppError::MissingUnqKey { row, .. } => {
                Some(Self::new(*row, "inv_unq_key", err.to_string()))
            }
            _ => None,
        }
    }
}
