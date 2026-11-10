// src-tauri/src/parser/bank_voucher.rs
//
// 职责：
//   1. 读取 Bank 进项发票凭证 Excel 的 Sheet1
//   2. 从第 2 行开始解析数据
//   3. 只提取：发票号码、入账日期、税额、凭证号码
//   4. 不访问数据库，不判断 header_id
//   5. 返回 ParsedBankVoucherRow 和解析错误

use crate::error::AppError;
use crate::models::dto::{BankVoucherErrorKind, BankVoucherImportError};
use crate::utils::{excel_date_to_string, normalize_number_string, yuan_to_fen};
use calamine::{open_workbook, Data, Reader, Xlsx};

/// Bank 文件列索引，0-based。
///
/// 表头：
/// A 购买方信息
/// B 开票日期
/// C 入账日期
/// D 发票号码
/// E 发票金额
/// F 税额
/// G 价税总计
/// H E+F-G
/// I 凭证号码
/// J 入账月份
/// K 入账年份
const COL_BUYER_INFO: usize = 0;
#[allow(dead_code)]
const COL_ISSUE_DATE: usize = 1;
const COL_POSTED_DATE: usize = 2;
const COL_INVOICE_NO: usize = 3;
#[allow(dead_code)]
const COL_AMOUNT: usize = 4;
const COL_TAX: usize = 5;
#[allow(dead_code)]
const COL_TOTAL: usize = 6;
#[allow(dead_code)]
const COL_DIFF: usize = 7;
const COL_VOUCHER_NO: usize = 8;
#[allow(dead_code)]
const COL_POSTED_MONTH: usize = 9;
#[allow(dead_code)]
const COL_POSTED_YEAR: usize = 10;

const COL_NAMES: &[&str] = &[
    "购买方信息",
    "开票日期",
    "入账日期",
    "发票号码",
    "发票金额",
    "税额",
    "价税总计",
    "E+F-G",
    "凭证号码",
    "入账月份",
    "入账年份",
];

fn col_name(idx: usize) -> &'static str {
    COL_NAMES.get(idx).copied().unwrap_or("未知列")
}

#[derive(Debug, Clone)]
pub struct ParsedBankVoucherRow {
    pub excel_row: u32,
    pub invoice_no: String,
    pub voucher_no: String,
    pub posted_date: String,
    pub tax_fen: i64,
}

/// 解析 Bank Excel。
///
/// 返回：
///   - Vec<ParsedBankVoucherRow>：可用于后续数据库匹配和写入的数据
///   - Vec<BankVoucherImportError>：Excel 解析阶段发现的错误
pub fn parse_bank_voucher_excel(
    file_path: &str,
) -> Result<(Vec<ParsedBankVoucherRow>, Vec<BankVoucherImportError>), AppError> {
    let mut workbook: Xlsx<_> = open_workbook(file_path)?;

    // Bank 文件固定读取 Sheet1
    let sheet = workbook
        .worksheet_range_at(0)
        .ok_or_else(|| AppError::SheetNotFound("第一个Sheet".to_string()))?
        .map_err(|_| AppError::SheetNotFound("第一个Sheet".to_string()))?;

    let rows: Vec<Vec<Data>> = sheet.rows().map(|r| r.to_vec()).collect();

    let mut parsed_rows: Vec<ParsedBankVoucherRow> = Vec::new();
    let mut errors: Vec<BankVoucherImportError> = Vec::new();

    // idx=0 是表头，从 idx=1 开始读取数据
    for (idx, row) in rows.iter().enumerate().skip(1) {
        let excel_row = (idx + 1) as u32;

        if is_empty_row(row) {
            continue;
        }

        // A 列出现合计时停止解析
        if total_row_check(row) {
            break;
        }

        match parse_bank_voucher_row(row, excel_row) {
            Ok(parsed) => parsed_rows.push(parsed),
            Err(err) => errors.push(err),
        }
    }

    Ok((parsed_rows, errors))
}

fn parse_bank_voucher_row(
    row: &[Data],
    excel_row: u32,
) -> Result<ParsedBankVoucherRow, BankVoucherImportError> {
    let invoice_no = match cell_to_opt_string(row.get(COL_INVOICE_NO)) {
        Some(v) => normalize_invoice_no(&v),
        None => {
            return Err(parse_error(
                excel_row,
                "",
                "",
                format!("必填字段「{}」为空", col_name(COL_INVOICE_NO)),
            ));
        }
    };

    if invoice_no.is_empty() {
        return Err(parse_error(
            excel_row,
            "",
            "",
            format!("必填字段「{}」为空", col_name(COL_INVOICE_NO)),
        ));
    }

    if invoice_no.len() != 8 && invoice_no.len() != 20 {
        return Err(parse_error(
            excel_row,
            &invoice_no,
            "",
            format!(
                "发票号码长度异常：应为 20 位数电票号码或 8 位传统发票号码，实际为 {} 位",
                invoice_no.len()
            ),
        ));
    }

    if !invoice_no.chars().all(|c| c.is_ascii_digit()) {
        return Err(parse_error(
            excel_row,
            &invoice_no,
            "",
            "发票号码格式错误：只能包含数字".to_string(),
        ));
    }

    let voucher_no = cell_to_opt_string(row.get(COL_VOUCHER_NO)).unwrap_or_default();

    if voucher_no.trim().is_empty() {
        return Err(parse_error(
            excel_row,
            &invoice_no,
            "",
            format!("必填字段「{}」为空", col_name(COL_VOUCHER_NO)),
        ));
    }
    // let posted_date = parse_posted_date_to_string(
    //     row.get(COL_POSTED_DATE),
    //     excel_row,
    //     &invoice_no,
    //     &voucher_no,
    // )?;
    let posted_date = match excel_date_to_string(row.get(COL_POSTED_DATE)) {
        Ok(date_str) => date_str,
        Err(err_msg) => {
            // 在这里将底层的转换错误，包装成带有行号、发票号等上下文的业务错误
            return Err(parse_error(
                excel_row,
                &invoice_no,
                &voucher_no,
                format!(
                    "必填字段「{}」解析失败：{}",
                    col_name(COL_POSTED_DATE),
                    err_msg
                ),
            ));
        }
    };
    // let posted_date = cell_to_opt_string(row.get(COL_POSTED_DATE));

    let tax_fen = match cell_to_opt_f64(row.get(COL_TAX)) {
        Some(v) => yuan_to_fen(v),
        None => {
            let tax_raw = cell_to_opt_string(row.get(COL_TAX)).unwrap_or_default();

            return Err(BankVoucherImportError {
                row: excel_row,
                invoice_no,
                voucher_no,
                kind: BankVoucherErrorKind::Parse,
                message: format!(
                    "税额格式错误：字段「{}」无法解析为数字，实际值为「{}」",
                    col_name(COL_TAX),
                    tax_raw
                ),
            });
        }
    };

    Ok(ParsedBankVoucherRow {
        excel_row,
        invoice_no,
        voucher_no,
        posted_date,
        tax_fen,
    })
}

fn parse_error(
    row: u32,
    invoice_no: &str,
    voucher_no: &str,
    message: String,
) -> BankVoucherImportError {
    BankVoucherImportError {
        row,
        invoice_no: invoice_no.to_string(),
        voucher_no: voucher_no.to_string(),
        kind: BankVoucherErrorKind::Parse,
        message,
    }
}

fn is_empty_row(row: &[Data]) -> bool {
    row.iter().all(|cell| match cell {
        Data::Empty => true,
        Data::String(s) => s.trim().is_empty(),
        _ => false,
    })
}

fn total_row_check(row: &[Data]) -> bool {
    cell_to_opt_string(row.get(COL_BUYER_INFO))
        .map(|s| s.contains("合计"))
        .unwrap_or(false)
}

/// calamine::Data -> Option<String>
fn cell_to_opt_string(cell: Option<&Data>) -> Option<String> {
    let cell = cell?;

    let value = match cell {
        Data::Empty => return None,
        Data::String(s) => s.trim().to_string(),
        Data::Float(n) => normalize_number_string(*n),
        Data::Int(n) => n.to_string(),
        Data::Bool(b) => b.to_string(),
        Data::DateTime(dt) => dt.to_string(),
        Data::DateTimeIso(s) => s.trim().to_string(),
        Data::DurationIso(s) => s.trim().to_string(),
        Data::Error(_) => return None,
    };

    let value = value.trim().to_string();

    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

/// calamine::Data -> Option<f64>
fn cell_to_opt_f64(cell: Option<&Data>) -> Option<f64> {
    let cell = cell?;

    match cell {
        Data::Float(n) => Some(*n),
        Data::Int(n) => Some(*n as f64),
        Data::String(s) => s.trim().replace(',', "").parse::<f64>().ok(),
        Data::DateTime(_) => None,
        Data::DateTimeIso(_) => None,
        Data::DurationIso(_) => None,
        Data::Bool(_) => None,
        Data::Empty => None,
        Data::Error(_) => None,
    }
}

/// 发票号码归一化。
///
/// 注意：
/// 如果 20 位数电票号码在 Excel 中被保存成数字，Excel 本身可能已经丢失精度。
/// 所以源文件中发票号码列最好保持文本格式。
fn normalize_invoice_no(raw: &str) -> String {
    raw.trim()
        .replace(' ', "")
        .replace('\u{00a0}', "")
        .replace('-', "")
}
