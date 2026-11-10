// src-tauri/src/parser/header.rs

use crate::error::{AppError, RowParseError};
use crate::models::db::{InvoiceHeader, ParsedHeader};
use crate::models::enums::{InvSource, InvState, InvType, IsPositive, RiskLevel};
use crate::models::raw_row::HeaderRaw;
use crate::utils::{
    build_unq_key, data_to_opt_str, extract_rev_key, usize_to_u32, yuan_to_fen_str,
};

use calamine::Data;
use std::str::FromStr;

const SHEET: &str = "发票基础信息";

// 列名映射（用于错误信息，与 Excel 列索引对应）
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

pub fn parse_header_sheet(range: &calamine::Range<Data>) -> HeaderParseResult {
    let mut rows = Vec::new();
    let mut errors = Vec::new();

    for (i, row) in range.rows().enumerate().skip(1) {
        let excel_row = i + 1;

        if row.iter().all(|c| matches!(c, Data::Empty)) {
            continue;
        }
        if let Some(Data::String(s)) = row.get(0) {
            if s.contains("合计") {
                continue;
            }
        }

        match parse_one_row(row, excel_row) {
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

fn parse_one_row(row: &[Data], row_idx: usize) -> Result<ParsedHeader, AppError> {
    let row_u32 = usize_to_u32(row_idx);

    // ── Step 1: Raw 层转换（类型安全，无业务语义）──────────────
    let raw = HeaderRaw::from_row(row).map_err(|(col_idx, _cell_err)| {
        // 根据列索引判断是缺失字段还是金额解析失败
        let col_name = COL_NAMES.get(col_idx).copied().unwrap_or("未知列");
        match col_idx {
            9 | 10 | 11 => AppError::AmountParse {
                sheet: SHEET,
                row: row_u32,
                col: col_name,
                value: data_to_opt_str(row.get(col_idx).unwrap_or(&Data::Empty))
                    .unwrap_or_default(),
            },
            _ => AppError::MissingField {
                sheet: SHEET,
                row: row_u32,
                col: col_name,
            },
        }
    })?;

    // ── Step 2: 枚举解析（业务语义）────────────────────────────
    let source: InvSource = InvSource::from_str(&raw.source).map_err(|_| AppError::EnumParse {
        sheet: SHEET,
        row: row_u32,
        col: "发票来源",
        value: raw.source.clone(),
    })?;

    let inv_type: InvType = InvType::from_str(&raw.inv_type).map_err(|_| AppError::EnumParse {
        sheet: SHEET,
        row: row_u32,
        col: "发票票种",
        value: raw.inv_type.clone(),
    })?;

    let state: InvState = InvState::from_str(&raw.state).map_err(|_| AppError::EnumParse {
        sheet: SHEET,
        row: row_u32,
        col: "发票状态",
        value: raw.state.clone(),
    })?;

    let is_pos: IsPositive =
        IsPositive::from_str(&raw.is_pos).map_err(|_| AppError::EnumParse {
            sheet: SHEET,
            row: row_u32,
            col: "是否正数发票",
            value: raw.is_pos.clone(),
        })?;

    let risk: RiskLevel = RiskLevel::from_str(&raw.risk).map_err(|_| AppError::EnumParse {
        sheet: SHEET,
        row: row_u32,
        col: "发票风险等级",
        value: raw.risk.clone(),
    })?;

    // ── Step 3: unq_key 生成 ────────────────────────────────────
    let unq_key = build_unq_key(
        raw.code.as_deref(),
        raw.num.as_deref(),
        raw.e_num.as_deref(),
    )
    .ok_or(AppError::MissingUnqKey {
        sheet: SHEET,
        row: row_u32,
    })?;

    // ── Step 4: 从备注提取红冲原始 key（仅红字发票有意义）──────
    // is_pos == No (0) 表示红字发票，但备注提取对所有行都尝试，
    // 有值则存，无值则 None，DB 层不做过滤。
    let rev_org_key = raw.remark.as_deref().and_then(extract_rev_key);

    // ── Step 5: 组装 ────────────────────────────────────────────
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
            amount: yuan_to_fen_str(&raw.amount),
            tax: yuan_to_fen_str(&raw.tax),
            total: yuan_to_fen_str(&raw.total),
            source: source as u8,
            inv_type: inv_type as u8,
            state: state as u8,
            is_pos: is_pos as u8,
            risk: risk as u8,
            issuer: raw.issuer,
            remark: raw.remark,
            rev_org_key,
            batch_no: 0,
        },
    })
}
