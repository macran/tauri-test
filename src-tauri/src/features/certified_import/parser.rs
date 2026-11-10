// src-tauri/src/features/certified_import/parser.rs

use calamine::{open_workbook, Data, Reader, Xlsx};

use crate::error::{AppError, RowParseError};
use crate::models::enums::CertRiskStatus;
use crate::utils::{build_unq_key, total_row_check, yuan_to_fen_str};

use super::models::{CertifiedRaw, ParsedCertifiedRow};

const SHEET_NAME: &str = "发票";

const COL_NAMES: &[&str] = &[
    "序号",               // 0
    "勾选状态",           // 1
    "发票来源",           // 2
    "转内销证明编号",     // 3
    "数电发票号码",       // 4
    "发票代码",           // 5
    "发票号码",           // 6
    "开票日期",           // 7
    "销售方纳税人识别号", // 8
    "销售方纳税人名称",   // 9
    "金额",               // 10
    "税额",               // 11
    "有效抵扣税额",       // 12
    "票种",               // 13
    "票种标签",           // 14
    "发票状态",           // 15
    "勾选时间",           // 16
    "发票风险等级",       // 17
    "风险状态",           // 18
];

fn col_name(idx: usize) -> &'static str {
    COL_NAMES.get(idx).copied().unwrap_or("未知列")
}

pub fn parse_certified_excel(
    file_path: &str,
) -> Result<(String, Vec<ParsedCertifiedRow>, Vec<RowParseError>), AppError> {
    // ── 打开 workbook ─────────────────────────────────────────
    let mut workbook: Xlsx<_> = open_workbook(file_path)?; // XlsxError → AppError::Excel

    let sheet = workbook
        .worksheet_range(SHEET_NAME)
        .map_err(|_| AppError::SheetNotFound(SHEET_NAME.to_string()))?;

    let rows: Vec<Vec<Data>> = sheet.rows().map(|r| r.to_vec()).collect();

    // ── 元信息行（idx=1）：F2 单元格提取税款所属期 ───────────
    let tax_period = rows
        .get(1)
        .and_then(|r| r.get(5))
        .and_then(|d| match d {
            Data::String(s) => {
                let t = s.trim();
                if t.is_empty() {
                    None
                } else {
                    Some(t.to_string())
                }
            }
            Data::Int(n) => Some(n.to_string()),
            Data::Float(f) => Some(format!("{}", *f as i64)),
            _ => None,
        })
        .unwrap_or_default();

    // ── 数据行（idx=3 起，idx=0/1 元信息区，idx=2 列头）─────
    let mut parsed_rows: Vec<ParsedCertifiedRow> = Vec::new();
    let mut errors: Vec<RowParseError> = Vec::new();

    for (idx, row) in rows.iter().enumerate().skip(3) {
        let excel_row = (idx + 1) as u32;

        // 跳过全空行
        if row.iter().all(|c| matches!(c, Data::Empty)) {
            continue;
        }
        if total_row_check(row) {
            continue;
        }
        // ── Step 1：提取原始字段 ──────────────────────────────
        let raw = match CertifiedRaw::from_row(row) {
            Ok(r) => r,
            Err((col_idx, _)) => {
                errors.push(RowParseError::new(
                    excel_row,
                    col_name(col_idx),
                    format!("必填字段「{}」为空", col_name(col_idx)),
                ));
                continue;
            }
        };

        // ── Step 2：生成 inv_unq_key ──────────────────────────
        let inv_unq_key = match build_unq_key(
            raw.code.as_deref(),
            raw.num.as_deref(),
            raw.e_num.as_deref(),
        ) {
            Some(k) => k,
            None => {
                errors.push(RowParseError::new(
                    excel_row,
                    "数电发票号码/发票代码/发票号码",
                    "无法生成发票唯一键：数电发票号码和传统发票代码+号码均为空",
                ));
                continue;
            }
        };

        // ── Step 3：金额转换（元 → 分）───────────────────────
        let valid_deduct_amount = match yuan_to_fen_str(&raw.valid_deduct_amount) {
            Ok(v) => v,
            Err(_) => {
                errors.push(RowParseError::new(
                    excel_row,
                    col_name(12),
                    format!(
                        "有效抵扣税额格式错误：「{}」无法解析为数字",
                        raw.valid_deduct_amount
                    ),
                ));
                continue;
            }
        };

        // ── Step 4：风险状态枚举解析 ──────────────────────────
        let risk_status = match CertRiskStatus::from_str(&raw.risk_status_raw) {
            Some(s) => s as u8,
            None => {
                errors.push(RowParseError::new(
                    excel_row,
                    col_name(18),
                    format!(
                        "风险状态「{}」无法识别，期望值：无风险/蓝色预警/黄色预警/红色预警",
                        raw.risk_status_raw
                    ),
                ));
                continue;
            }
        };

        parsed_rows.push(ParsedCertifiedRow {
            inv_unq_key,
            excel_row,
            domestic_proof_no: raw.domestic_proof_no,
            valid_deduct_amount,
            invoice_tag: raw.invoice_tag,
            check_time: raw.check_time,
            risk_status,
        });
    }

    Ok((tax_period, parsed_rows, errors))
}
