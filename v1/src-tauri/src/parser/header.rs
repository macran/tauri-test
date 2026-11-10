use crate::error::{AppError, RowParseError};
use crate::excel_reader::{SheetData, SheetRow};
use crate::models::db::{InvoiceHeader, ParsedHeader};
use crate::models::enums::{InvSource, InvState, InvType, IsPositive, RiskLevel};
use crate::utils::{
    build_unq_key, extract_red_invoice_reference, is_empty_row, is_total_row, opt_str, req_amount_str, req_str,
    usize_to_u32, yuan_to_fen_str,
};

use std::str::FromStr;

const SHEET: &str = "发票基础信息";

// 与 Excel 列顺序保持一致
const COL_NAMES: &[&str] = &[
    "序号",         // 0
    "发票代码",     // 1
    "发票号码",     // 2
    "数电发票号码", // 3
    "销方识别号",   // 4
    "销方名称",     // 5
    "购方识别号",   // 6
    "购买方名称",   // 7
    "开票日期",     // 8
    "金额",         // 9
    "税额",         // 10
    "价税合计",     // 11
    "发票来源",     // 12
    "发票票种",     // 13
    "发票状态",     // 14
    "是否正数发票", // 15
    "发票风险等级", // 16
    "开票人",       // 17
    "备注",         // 18
];

pub struct HeaderParseResult {
    pub rows: Vec<ParsedHeader>,
    pub errors: Vec<RowParseError>,
}

#[derive(Debug, Clone)]
struct HeaderRaw {
    code: Option<String>,
    num: Option<String>,
    e_num: Option<String>,
    seller_tax_no: String,
    seller_name: String,
    issue_date: String,
    amount: String,
    tax: String,
    total: String,
    source: String,
    inv_type: String,
    state: String,
    is_pos: String,
    risk: String,
    issuer: Option<String>,
    remark: Option<String>,
}

pub fn parse_header_sheet(sheet: &SheetData, shared: &[String]) -> HeaderParseResult {
    let mut rows = Vec::new();
    let mut errors = Vec::new();

    for (i, row) in sheet.data_rows().iter().enumerate() {
        let excel_row = i + 2;

        if is_empty_row(row) {
            continue;
        }

        if is_total_row(row, shared) {
            continue;
        }

        match parse_one_row(row, excel_row, shared) {
            Ok(parsed) => rows.push(parsed),
            Err(e) => {
                if let Some(re) = RowParseError::from_app_error(&e) {
                    errors.push(re);
                }
            }
        }
    }

    HeaderParseResult { rows, errors }
}

fn parse_one_row(
    row: &SheetRow,
    row_idx: usize,
    shared: &[String],
) -> Result<ParsedHeader, AppError> {
    let row_u32 = usize_to_u32(row_idx);

    let raw = HeaderRaw {
        code: opt_str(row, 1, shared),
        num: opt_str(row, 2, shared),
        e_num: opt_str(row, 3, shared),

        seller_tax_no: req_str(row, 4, shared).map_err(|(col_idx, _)| AppError::MissingField {
            sheet: SHEET,
            row: row_u32,
            col: COL_NAMES.get(col_idx).copied().unwrap_or("未知列"),
        })?,

        seller_name: req_str(row, 5, shared).map_err(|(col_idx, _)| AppError::MissingField {
            sheet: SHEET,
            row: row_u32,
            col: COL_NAMES.get(col_idx).copied().unwrap_or("未知列"),
        })?,

        issue_date: req_str(row, 8, shared).map_err(|(col_idx, _)| AppError::MissingField {
            sheet: SHEET,
            row: row_u32,
            col: COL_NAMES.get(col_idx).copied().unwrap_or("未知列"),
        })?,

        amount: req_amount_str(row, 9, shared).map_err(|(col_idx, _)| AppError::MissingField {
            sheet: SHEET,
            row: row_u32,
            col: COL_NAMES.get(col_idx).copied().unwrap_or("未知列"),
        })?,

        tax: req_amount_str(row, 10, shared).map_err(|(col_idx, _)| AppError::MissingField {
            sheet: SHEET,
            row: row_u32,
            col: COL_NAMES.get(col_idx).copied().unwrap_or("未知列"),
        })?,

        total: req_amount_str(row, 11, shared).map_err(|(col_idx, _)| AppError::MissingField {
            sheet: SHEET,
            row: row_u32,
            col: COL_NAMES.get(col_idx).copied().unwrap_or("未知列"),
        })?,

        source: req_str(row, 12, shared).map_err(|(col_idx, _)| AppError::MissingField {
            sheet: SHEET,
            row: row_u32,
            col: COL_NAMES.get(col_idx).copied().unwrap_or("未知列"),
        })?,

        inv_type: req_str(row, 13, shared).map_err(|(col_idx, _)| AppError::MissingField {
            sheet: SHEET,
            row: row_u32,
            col: COL_NAMES.get(col_idx).copied().unwrap_or("未知列"),
        })?,

        state: req_str(row, 14, shared).map_err(|(col_idx, _)| AppError::MissingField {
            sheet: SHEET,
            row: row_u32,
            col: COL_NAMES.get(col_idx).copied().unwrap_or("未知列"),
        })?,

        is_pos: req_str(row, 15, shared).map_err(|(col_idx, _)| AppError::MissingField {
            sheet: SHEET,
            row: row_u32,
            col: COL_NAMES.get(col_idx).copied().unwrap_or("未知列"),
        })?,

        risk: req_str(row, 16, shared).map_err(|(col_idx, _)| AppError::MissingField {
            sheet: SHEET,
            row: row_u32,
            col: COL_NAMES.get(col_idx).copied().unwrap_or("未知列"),
        })?,

        issuer: opt_str(row, 17, shared),
        remark: opt_str(row, 18, shared),
    };

    let source: InvSource = InvSource::from_str(&raw.source).map_err(|_| AppError::EnumParse {
        sheet: SHEET,
        row: row_u32,
        col: COL_NAMES[12],
        value: raw.source.clone(),
    })?;

    let inv_type: InvType = InvType::from_str(&raw.inv_type).map_err(|_| AppError::EnumParse {
        sheet: SHEET,
        row: row_u32,
        col: COL_NAMES[13],
        value: raw.inv_type.clone(),
    })?;

    let state: InvState = InvState::from_str(&raw.state).map_err(|_| AppError::EnumParse {
        sheet: SHEET,
        row: row_u32,
        col: COL_NAMES[14],
        value: raw.state.clone(),
    })?;

    let is_pos: IsPositive =
        IsPositive::from_str(&raw.is_pos).map_err(|_| AppError::EnumParse {
            sheet: SHEET,
            row: row_u32,
            col: COL_NAMES[15],
            value: raw.is_pos.clone(),
        })?;

    let risk: RiskLevel = RiskLevel::from_str(&raw.risk).map_err(|_| AppError::EnumParse {
        sheet: SHEET,
        row: row_u32,
        col: COL_NAMES[16],
        value: raw.risk.clone(),
    })?;

    let unq_key = build_unq_key(
        raw.code.as_deref(),
        raw.num.as_deref(),
        raw.e_num.as_deref(),
    )
    .ok_or(AppError::MissingUnqKey {
        sheet: SHEET,
        row: row_u32,
    })?;

    let red_invoice_reference = raw
    .remark
    .as_deref()
    .and_then(extract_red_invoice_reference);

let rev_org_key = red_invoice_reference
    .as_ref()
    .map(|reference| reference.blue_invoice_key.clone());

let red_notice_no = red_invoice_reference
    .and_then(|reference| reference.red_notice_no);


    let amount = yuan_to_fen_str(&raw.amount).map_err(|_| AppError::AmountParse {
        sheet: SHEET,
        row: row_u32,
        col: COL_NAMES[9],
        value: raw.amount.clone(),
    })?;

    let tax = yuan_to_fen_str(&raw.tax).map_err(|_| AppError::AmountParse {
        sheet: SHEET,
        row: row_u32,
        col: COL_NAMES[10],
        value: raw.tax.clone(),
    })?;

    let total = yuan_to_fen_str(&raw.total).map_err(|_| AppError::AmountParse {
        sheet: SHEET,
        row: row_u32,
        col: COL_NAMES[11],
        value: raw.total.clone(),
    })?;

    Ok(ParsedHeader {
        unq_key,
        seller_tax_no: raw.seller_tax_no.clone(),
        seller_name: raw.seller_name.clone(),
        excel_row: row_idx,
        data: InvoiceHeader {
            code: raw.code,
            num: raw.num,
            e_num: raw.e_num,
            seller_id: 0,
            issue_date: raw.issue_date,
            amount,
            tax,
            total,
            source: source as u8,
            inv_type: inv_type as u8,
            state: state as u8,
            is_pos: is_pos as u8,
            risk: risk as u8,
            issuer: raw.issuer,
            remark: raw.remark,
            rev_org_key,
            red_notice_no,
            batch_no: 0,
        },
    })
}
