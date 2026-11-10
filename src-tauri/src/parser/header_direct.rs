=use std::{collections::HashMap, str::FromStr, sync::Arc};

use anyhow::{anyhow, Context, Result};
use quick_xml::{
    events::{BytesStart, Event},
    Reader,
};

use crate::{
    error::{AppError, RowParseError},
    models::{
        db::{InvoiceHeader, ParsedHeader},
        enums::{InvSource, InvState, InvType, IsPositive, RiskLevel},
    },
    utils::{build_unq_key, extract_rev_key, usize_to_u32, yuan_to_fen_str},
};

const SHEET: &str = "发票基础信息";

const COL_CODE: usize = 1;
const COL_NUM: usize = 2;
const COL_E_NUM: usize = 3;
const COL_SELLER_TAX_NO: usize = 4;
const COL_SELLER_NAME: usize = 5;
const COL_ISSUE_DATE: usize = 8;
const COL_AMOUNT: usize = 9;
const COL_TAX: usize = 10;
const COL_TOTAL: usize = 11;
const COL_SOURCE: usize = 12;
const COL_INV_TYPE: usize = 13;
const COL_STATE: usize = 14;
const COL_IS_POS: usize = 15;
const COL_RISK: usize = 16;
const COL_ISSUER: usize = 17;
const COL_REMARK: usize = 18;

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

#[derive(Debug, Default)]
struct HeaderRowBuilder {
    code: Option<String>,
    num: Option<String>,
    e_num: Option<String>,
    seller_tax_no: Option<String>,
    seller_name: Option<String>,
    issue_date: Option<String>,
    amount: Option<String>,
    tax: Option<String>,
    total: Option<String>,
    source: Option<String>,
    inv_type: Option<String>,
    state: Option<String>,
    is_pos: Option<String>,
    risk: Option<String>,
    issuer: Option<String>,
    remark: Option<String>,
}

impl HeaderRowBuilder {
    #[inline]
    fn set_cell(&mut self, col_idx: usize, value: String) {
        let v = if value.trim().is_empty() {
            None
        } else {
            Some(value.trim().to_string())
        };

        match col_idx {
            COL_CODE => self.code = v,
            COL_NUM => self.num = v,
            COL_E_NUM => self.e_num = v,
            COL_SELLER_TAX_NO => self.seller_tax_no = v,
            COL_SELLER_NAME => self.seller_name = v,
            COL_ISSUE_DATE => self.issue_date = v,
            COL_AMOUNT => self.amount = v,
            COL_TAX => self.tax = v,
            COL_TOTAL => self.total = v,
            COL_SOURCE => self.source = v,
            COL_INV_TYPE => self.inv_type = v,
            COL_STATE => self.state = v,
            COL_IS_POS => self.is_pos = v,
            COL_RISK => self.risk = v,
            COL_ISSUER => self.issuer = v,
            COL_REMARK => self.remark = v,
            _ => {}
        }
    }

    #[inline]
    fn is_effectively_empty(&self) -> bool {
        self.code.is_none()
            && self.num.is_none()
            && self.e_num.is_none()
            && self.seller_tax_no.is_none()
            && self.seller_name.is_none()
            && self.issue_date.is_none()
            && self.amount.is_none()
            && self.tax.is_none()
            && self.total.is_none()
            && self.source.is_none()
            && self.inv_type.is_none()
            && self.state.is_none()
            && self.is_pos.is_none()
            && self.risk.is_none()
            && self.issuer.is_none()
            && self.remark.is_none()
    }
}

pub fn parse_header_sheet_direct(
    xml: &[u8],
    shared_strings: &[Arc<str>],
) -> HeaderParseResult {
    match parse_header_sheet_direct_inner(xml, shared_strings) {
        Ok(v) => v,
        Err(e) => HeaderParseResult {
            rows: vec![],
            errors: vec![RowParseError {
                sheet: SHEET.to_string(),
                row: 0,
                col: String::new(),
                message: e.to_string(),
            }],
        },
    }
}

fn parse_header_sheet_direct_inner(
    xml: &[u8],
    shared_strings: &[Arc<str>],
) -> Result<HeaderParseResult> {
    let mut reader = Reader::from_reader(std::io::Cursor::new(xml));
    reader.config_mut().trim_text(false);

    let mut buf = Vec::new();
    let mut scratch = String::new();

    let mut results = Vec::new();
    let mut errors = Vec::new();

    let mut current_row_no: usize = 0; // Excel 行号，1-based
    let mut in_row = false;
    let mut row_builder = HeaderRowBuilder::default();

    let mut cur_col_idx: usize = 0;
    let mut cur_cell_type = CellType::Number;
    let mut in_value = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) if e.name().as_ref() == b"row" => {
                current_row_no = parse_row_index(e).unwrap_or(current_row_no + 1);
                in_row = true;
                row_builder = HeaderRowBuilder::default();
            }

            Ok(Event::End(ref e)) if e.name().as_ref() == b"row" => {
                if in_row {
                    // 第 1 行表头跳过
                    if current_row_no > 1 && !row_builder.is_effectively_empty() {
                        match build_parsed_header(row_builder, current_row_no) {
                            Ok(parsed) => results.push(parsed),
                            Err(err) => {
                                if let Some(re) = RowParseError::from_app_error(&err) {
                                    errors.push(re);
                                } else {
                                    errors.push(RowParseError {
                                        sheet: SHEET.to_string(),
                                        row: usize_to_u32(current_row_no),
                                        col: String::new(),
                                        message: err.to_string(),
                                    });
                                }
                            }
                        }
                    }
                    in_row = false;
                }
            }

            Ok(Event::Start(ref e)) if e.name().as_ref() == b"c" => {
                scratch.clear();
                let (col_idx, cell_type) = parse_cell_attrs(e)?;
                cur_col_idx = col_idx;
                cur_cell_type = cell_type;
            }

            Ok(Event::Empty(ref e)) if e.name().as_ref() == b"c" => {
                let (col_idx, _cell_type) = parse_cell_attrs(e)?;
                if is_needed_col(col_idx) {
                    row_builder.set_cell(col_idx, String::new());
                }
            }

            Ok(Event::Start(ref e)) if matches!(e.name().as_ref(), b"v" | b"t") => {
                in_value = true;
                scratch.clear();
            }

            Ok(Event::Text(ref text)) if in_value => {
                scratch.push_str(
                    &text
                        .xml_content()
                        .context("failed to decode header sheet text")?,
                );
            }

            Ok(Event::CData(ref text)) if in_value => {
                scratch.push_str(
                    &text
                        .xml_content()
                        .context("failed to decode header sheet cdata")?,
                );
            }

            Ok(Event::End(ref e)) if in_value && matches!(e.name().as_ref(), b"v" | b"t") => {
                in_value = false;

                if is_needed_col(cur_col_idx) {
                    let value = resolve_cell_string(&scratch, cur_cell_type, shared_strings);
                    row_builder.set_cell(cur_col_idx, value);
                }
            }

            Ok(Event::End(ref e)) if e.name().as_ref() == b"c" => {
                cur_cell_type = CellType::Number;
                scratch.clear();
            }

            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(err) => return Err(anyhow!(err)).context("failed to parse header sheet xml"),
        }

        buf.clear();
    }

    Ok(HeaderParseResult {
        rows: results,
        errors,
    })
}

fn build_parsed_header(
    raw: HeaderRowBuilder,
    row_idx: usize,
) -> Result<ParsedHeader, AppError> {
    let row_u32 = usize_to_u32(row_idx);

    let seller_tax_no = raw.seller_tax_no.ok_or(AppError::MissingField {
        sheet: SHEET,
        row: row_u32,
        col: COL_NAMES[COL_SELLER_TAX_NO],
    })?;

    let seller_name = raw.seller_name.ok_or(AppError::MissingField {
        sheet: SHEET,
        row: row_u32,
        col: COL_NAMES[COL_SELLER_NAME],
    })?;

    let issue_date = raw.issue_date.ok_or(AppError::MissingField {
        sheet: SHEET,
        row: row_u32,
        col: COL_NAMES[COL_ISSUE_DATE],
    })?;

    let amount_raw = raw.amount.ok_or(AppError::MissingField {
        sheet: SHEET,
        row: row_u32,
        col: COL_NAMES[COL_AMOUNT],
    })?;

    let tax_raw = raw.tax.ok_or(AppError::MissingField {
        sheet: SHEET,
        row: row_u32,
        col: COL_NAMES[COL_TAX],
    })?;

    let total_raw = raw.total.ok_or(AppError::MissingField {
        sheet: SHEET,
        row: row_u32,
        col: COL_NAMES[COL_TOTAL],
    })?;

    let source_raw = raw.source.ok_or(AppError::MissingField {
        sheet: SHEET,
        row: row_u32,
        col: COL_NAMES[COL_SOURCE],
    })?;

    let inv_type_raw = raw.inv_type.ok_or(AppError::MissingField {
        sheet: SHEET,
        row: row_u32,
        col: COL_NAMES[COL_INV_TYPE],
    })?;

    let state_raw = raw.state.ok_or(AppError::MissingField {
        sheet: SHEET,
        row: row_u32,
        col: COL_NAMES[COL_STATE],
    })?;

    let is_pos_raw = raw.is_pos.ok_or(AppError::MissingField {
        sheet: SHEET,
        row: row_u32,
        col: COL_NAMES[COL_IS_POS],
    })?;

    let risk_raw = raw.risk.ok_or(AppError::MissingField {
        sheet: SHEET,
        row: row_u32,
        col: COL_NAMES[COL_RISK],
    })?;

    let source = InvSource::from_str(&source_raw).map_err(|_| AppError::EnumParse {
        sheet: SHEET,
        row: row_u32,
        col: COL_NAMES[COL_SOURCE],
        value: source_raw.clone(),
    })?;

    let inv_type = InvType::from_str(&inv_type_raw).map_err(|_| AppError::EnumParse {
        sheet: SHEET,
        row: row_u32,
        col: COL_NAMES[COL_INV_TYPE],
        value: inv_type_raw.clone(),
    })?;

    let state = InvState::from_str(&state_raw).map_err(|_| AppError::EnumParse {
        sheet: SHEET,
        row: row_u32,
        col: COL_NAMES[COL_STATE],
        value: state_raw.clone(),
    })?;

    let is_pos = IsPositive::from_str(&is_pos_raw).map_err(|_| AppError::EnumParse {
        sheet: SHEET,
        row: row_u32,
        col: COL_NAMES[COL_IS_POS],
        value: is_pos_raw.clone(),
    })?;

    let risk = RiskLevel::from_str(&risk_raw).map_err(|_| AppError::EnumParse {
        sheet: SHEET,
        row: row_u32,
        col: COL_NAMES[COL_RISK],
        value: risk_raw.clone(),
    })?;

    let amount = yuan_to_fen_str(&amount_raw).map_err(|_| AppError::AmountParse {
        sheet: SHEET,
        row: row_u32,
        col: COL_NAMES[COL_AMOUNT],
        value: amount_raw.clone(),
    })?;

    let tax = yuan_to_fen_str(&tax_raw).map_err(|_| AppError::AmountParse {
        sheet: SHEET,
        row: row_u32,
        col: COL_NAMES[COL_TAX],
        value: tax_raw.clone(),
    })?;

    let total = yuan_to_fen_str(&total_raw).map_err(|_| AppError::AmountParse {
        sheet: SHEET,
        row: row_u32,
        col: COL_NAMES[COL_TOTAL],
        value: total_raw.clone(),
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

    let rev_org_key = raw.remark.as_deref().and_then(extract_rev_key);

    Ok(ParsedHeader {
        unq_key,
        seller_tax_no,
        seller_name,
        excel_row: row_idx,
        data: InvoiceHeader {
            code: raw.code,
            num: raw.num,
            e_num: raw.e_num,
            seller_id: 0,
            issue_date,
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
            batch_no: 0,
        },
    })
}

#[inline]
fn is_needed_col(col_idx: usize) -> bool {
    matches!(
        col_idx,
        COL_CODE
            | COL_NUM
            | COL_E_NUM
            | COL_SELLER_TAX_NO
            | COL_SELLER_NAME
            | COL_ISSUE_DATE
            | COL_AMOUNT
            | COL_TAX
            | COL_TOTAL
            | COL_SOURCE
            | COL_INV_TYPE
            | COL_STATE
            | COL_IS_POS
            | COL_RISK
            | COL_ISSUER
            | COL_REMARK
    )
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum CellType {
    Number,
    SharedString,
    InlineStr,
    Str,
    Bool,
}

#[inline]
fn parse_cell_attrs(e: &BytesStart<'_>) -> Result<(usize, CellType)> {
    let mut col_idx = 0usize;
    let mut cell_type = CellType::Number;

    for attr in e.attributes() {
        let attr = attr.context("failed to read cell attribute")?;
        match attr.key.as_ref() {
            b"r" => {
                col_idx = col_ref_to_idx_bytes(attr.value.as_ref());
            }
            b"t" => {
                cell_type = match attr.value.as_ref() {
                    b"s" => CellType::SharedString,
                    b"inlineStr" => CellType::InlineStr,
                    b"str" => CellType::Str,
                    b"b" => CellType::Bool,
                    _ => CellType::Number,
                };
            }
            _ => {}
        }
    }

    Ok((col_idx, cell_type))
}

#[inline]
fn parse_row_index(e: &BytesStart<'_>) -> Option<usize> {
    for attr in e.attributes().flatten() {
        if attr.key.as_ref() == b"r" {
            let bytes = attr.value.as_ref();
            if let Ok(s) = std::str::from_utf8(bytes) {
                if let Ok(v) = s.parse::<usize>() {
                    return Some(v);
                }
            }
        }
    }
    None
}

#[inline]
fn col_ref_to_idx_bytes(cell_ref: &[u8]) -> usize {
    let mut col = 0usize;
    for &b in cell_ref {
        if b.is_ascii_alphabetic() {
            let u = b.to_ascii_uppercase();
            col = col * 26 + (u - b'A') as usize + 1;
        } else {
            break;
        }
    }
    col.saturating_sub(1)
}

#[inline]
fn resolve_cell_string(
    scratch: &str,
    cell_type: CellType,
    shared_strings: &[Arc<str>],
) -> String {
    match cell_type {
        CellType::SharedString => scratch
            .trim()
            .parse::<usize>()
            .ok()
            .and_then(|i| shared_strings.get(i))
            .map(|s| s.to_string())
            .unwrap_or_default(),
        CellType::InlineStr | CellType::Str | CellType::Bool => scratch.to_string(),
        CellType::Number => scratch.trim().to_string(),
    }
}
