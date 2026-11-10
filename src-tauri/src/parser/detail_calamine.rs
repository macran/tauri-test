// src-tauri/src/parser/detail.rs

use calamine::Data;
use std::str::FromStr;

use crate::error::{AppError, RowParseError};
use crate::models::db::{InvoiceDetail, ParsedInvoiceDetail};
use crate::models::enums::SpecialBizType;
use crate::models::raw_row::DetailRaw;
use crate::utils::{
    build_unq_key, data_to_opt_str, parse_goods_name, usize_to_u32, yuan_to_fen_str,
};

const SHEET: &str = "信息汇总表";

const COL_NAMES: &[&str] = &[
    "序号",               // 0
    "发票代码",           // 1
    "发票号码",           // 2
    "数电发票号码",       // 3
    "",                   // 4 不入库
    "",                   // 5 不入库
    "",                   // 6 不入库
    "",                   // 7 不入库
    "",                   // 8 不入库
    "税收分类编码",       // 9
    "特定业务类型",       // 10
    "货物或应税劳务名称", // 11
    "规格型号",           // 12
    "单位",               // 13
    "数量",               // 14
    "单价",               // 15
    "金额",               // 16
    "税率",               // 17
    "税额",               // 18
    "价税合计",           // 19
];

pub struct DetailParseResult {
    pub rows: Vec<ParsedInvoiceDetail>,
    pub errors: Vec<RowParseError>,
}

pub fn parse_detail_sheet(range: &calamine::Range<Data>) -> DetailParseResult {
    let mut rows = Vec::new();
    let mut errors = Vec::new();

    for (i, row) in range.rows().enumerate().skip(1) {
        let excel_row = i + 1;

        if row.iter().all(|c| matches!(c, Data::Empty)) {
            continue;
        }
        match row.get(0) {
            Some(Data::String(s)) if s.contains("合计") => continue,
            Some(Data::Empty) => continue,
            _ => {}
        }

        match parse_one_row(row, excel_row) {
            Ok(result) => rows.push(result),
            Err(e) => {
                if let Some(re) = RowParseError::from_app_error(&e) {
                    errors.push(re);
                }
            }
        }
    }

    DetailParseResult { rows, errors }
}

fn parse_one_row(row: &[Data], row_idx: usize) -> Result<ParsedInvoiceDetail, AppError> {
    let row_u32 = usize_to_u32(row_idx);

    // ── Step 1: Raw 层转换 ──────────────────────────────────────
    let raw = DetailRaw::from_row(row).map_err(|(col_idx, _)| {
        let col_name = COL_NAMES.get(col_idx).copied().unwrap_or("未知列");
        match col_idx {
            16 | 18 | 19 => AppError::AmountParse {
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

    // ── Step 2: unq_key ─────────────────────────────────────────
    let unq_key = build_unq_key(
        raw.code.as_deref(),
        raw.num.as_deref(),
        raw.e_num.as_deref(),
    )
    .ok_or(AppError::MissingUnqKey {
        sheet: SHEET,
        row: row_u32,
    })?;

    // ── Step 3: 特定业务类型枚举解析 ────────────────────────────
    let biz_type: Option<u8> = match raw.special_biz {
        None => None,
        Some(ref s) if s.trim().is_empty() => None,
        Some(ref s) => {
            let biz = SpecialBizType::from_str(s).map_err(|_| AppError::EnumParse {
                sheet: SHEET,
                row: row_u32,
                col: "特定业务类型",
                value: s.clone(),
            })?;
            Some(biz as u8)
        }
    };

    // ── Step 4: 货物名称解析（*类别*名称 格式）──────────────────
    let (goods_category, goods_name) = parse_goods_name(&raw.goods_name);

    // ── Step 5: 组装 ────────────────────────────────────────────
    Ok(ParsedInvoiceDetail {
        unq_key,
        excel_row: row_idx,
        data: InvoiceDetail {
            tax_code: raw.tax_cls_code,
            biz_type,

            goods_category,
            goods_name,
            spec_model: raw.spec,
            unit: raw.unit,
            quantity: raw.quantity,
            unit_price: raw.unit_price,
            amount: yuan_to_fen_str(&raw.amount),
            tax_rate: raw.tax_rate,
            tax: yuan_to_fen_str(&raw.tax),
            total: yuan_to_fen_str(&raw.total),
        },
    })
}
