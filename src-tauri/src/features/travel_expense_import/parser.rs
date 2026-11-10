// src-tauri/src/features/travel_expense_import/parser.rs
//
// 职责：
//   1. 读取 Travel Expense Excel 第一个 Sheet
//   2. 从第二行开始解析数据
//   3. 只提取：数电票号码、发票代码、发票号码、凭证号码、入账时间
//   4. 按统一规则生成 inv_unq_key
//   5. 返回 ParsedTravelExpenseRow 和解析错误

use calamine::{open_workbook, Data, Reader, Xlsx};

use crate::error::AppError;
use crate::utils::{
    build_unq_key, c_is_empty_row, cell_to_opt_string, excel_date_to_string, total_row_check,
};

use super::models::{ParsedTravelExpenseRow, TravelExpenseImportError};

/// Travel Expense 文件列索引，0-based。
///
/// 表头顺序：
/// 序号 是否勾选* 数电票号码 发票代码 发票号码 开票日期* 金额*
/// 票面税额* 有效抵扣税额* 购买方识别号* 销售方纳税人名称 销售方纳税人识别号
/// 发票来源 票种* 发票状态 红字锁定标志 转内销证明编号 业务类型
/// 勾选时间 发票风险等级 风险状态 凭证号码 入账时间
const COL_E_NUM: usize = 2; // 数电票号码
const COL_CODE: usize = 3; // 发票代码
const COL_NUM: usize = 4; // 发票号码
const COL_VOUCHER_NO: usize = 21; // 凭证号码
const COL_POSTED_DATE: usize = 22; // 入账时间

const COL_NAMES: &[&str] = &[
    "序号",               // 0
    "是否勾选*",          // 1
    "数电票号码",         // 2
    "发票代码",           // 3
    "发票号码",           // 4
    "开票日期*",          // 5
    "金额*",              // 6
    "票面税额*",          // 7
    "有效抵扣税额*",      // 8
    "购买方识别号*",      // 9
    "销售方纳税人名称",   // 10
    "销售方纳税人识别号", // 11
    "发票来源",           // 12
    "票种*",              // 13
    "发票状态",           // 14
    "红字锁定标志",       // 15
    "转内销证明编号",     // 16
    "业务类型",           // 17
    "勾选时间",           // 18
    "发票风险等级",       // 19
    "风险状态",           // 20
    "凭证号码",           // 21
    "入账时间",           // 22
];

fn col_name(idx: usize) -> &'static str {
    COL_NAMES.get(idx).copied().unwrap_or("未知列")
}

/// 解析 Travel Expense Excel。
///
/// 返回：
///   - Vec<ParsedTravelExpenseRow>：可用于更新数据库的数据
///   - Vec<TravelExpenseImportError>：解析阶段发现的错误
pub fn parse_travel_expense_excel(
    file_path: &str,
) -> Result<(Vec<ParsedTravelExpenseRow>, Vec<TravelExpenseImportError>), AppError> {
    // ── 打开 workbook ─────────────────────────────────────────
    let mut workbook: Xlsx<_> = open_workbook(file_path)?;

    // Travel Expense 文件只有一个 Sheet，因此直接取第一个 Sheet
    let sheet_name = workbook
        .sheet_names()
        .first()
        .cloned()
        .ok_or_else(|| AppError::SheetNotFound("第一个Sheet".to_string()))?;

    let sheet = workbook
        .worksheet_range(&sheet_name)
        .map_err(|_| AppError::SheetNotFound(sheet_name.clone()))?;

    let rows: Vec<Vec<Data>> = sheet.rows().map(|r| r.to_vec()).collect();

    let mut parsed_rows: Vec<ParsedTravelExpenseRow> = Vec::new();
    let mut errors: Vec<TravelExpenseImportError> = Vec::new();

    // ── 数据行：idx=0 是表头，idx=1 开始是数据 ───────────────
    for (idx, row) in rows.iter().enumerate().skip(1) {
        let excel_row = (idx + 1) as u32;

        // 跳过空行
        if c_is_empty_row(row) {
            continue;
        }

        // A 列如果出现“合计”，停止解析
        // 注意：这里是 break，不是 continue。
        if total_row_check(row) {
            break;
        }

        let parsed = match parse_travel_expense_row(row, excel_row) {
            Ok(v) => v,
            Err(err) => {
                errors.push(err);
                continue;
            }
        };

        parsed_rows.push(parsed);
    }

    Ok((parsed_rows, errors))
}

/// 解析单行 Travel Expense 数据。
fn parse_travel_expense_row(
    row: &[Data],
    excel_row: u32,
) -> Result<ParsedTravelExpenseRow, TravelExpenseImportError> {
    // ── Step 1：提取生成 inv_unq_key 所需字段 ────────────────
    let e_num = cell_to_opt_string(row.get(COL_E_NUM));
    let code = cell_to_opt_string(row.get(COL_CODE));
    let num = cell_to_opt_string(row.get(COL_NUM));

    // ── Step 2：提取需要更新的字段 ───────────────────────────
    let voucher_no = cell_to_opt_string(row.get(COL_VOUCHER_NO)).unwrap_or_default();
    let posted_date = match excel_date_to_string(row.get(COL_POSTED_DATE)) {
        Ok(date_str) => date_str,
        Err(err_msg) => {
            return Err(TravelExpenseImportError {
                row: excel_row,
                inv_unq_key: String::new(),
                voucher_no: voucher_no.clone(),
                message: format!(
                    "必填字段「{}」解析失败：{}",
                    col_name(COL_POSTED_DATE),
                    err_msg
                ),
            }); // ← 这里：}) 改为 }); 去掉多余的右括号
        }
    };

    // ── Step 3：生成 inv_unq_key ─────────────────────────────
    let inv_unq_key = match build_unq_key(code.as_deref(), num.as_deref(), e_num.as_deref()) {
        Some(k) => k,
        None => {
            return Err(TravelExpenseImportError {
                row: excel_row,
                inv_unq_key: String::new(),
                voucher_no,
                message: format!(
                    "无法生成发票唯一键：{}、{}、{}均为空或无效",
                    col_name(COL_E_NUM),
                    col_name(COL_CODE),
                    col_name(COL_NUM)
                ),
            });
        }
    };

    // ── Step 4：凭证号码校验 ─────────────────────────────────
    //
    // 如果你的业务允许凭证号码为空并更新 NULL，可以删掉这个判断。
    if voucher_no.trim().is_empty() {
        return Err(TravelExpenseImportError {
            row: excel_row,
            inv_unq_key,
            voucher_no,
            message: format!("必填字段「{}」为空", col_name(COL_VOUCHER_NO)),
        });
    }

    Ok(ParsedTravelExpenseRow {
        excel_row,
        inv_unq_key,
        voucher_no,
        posted_date,
    })
}
