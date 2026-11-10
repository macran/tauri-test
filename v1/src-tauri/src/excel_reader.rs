// src-tauri/src/excel_reader.rs
//
// 用 zip + quick-xml 直接解析 xlsx，替换 calamine。
// 核心优势：sharedStrings 只解析一次，各 sheet XML rayon 并行解析。
//
// 对外接口和原来 calamine Range<Data> 的使用方式保持一致：
//   - SheetData.rows[0]          → 表头行
//   - SheetData.rows[1..]        → 数据行
//   - row.get_string(col_index)  → 取字符串
//   - row.get_f64(col_index)     → 取数字

use crate::error::AppError;
use anyhow::{anyhow, Context, Result};
use quick_xml::{
    events::{BytesStart, Event},
    Reader,
};
use rayon::prelude::*;
use std::{
    collections::{HashMap, HashSet},
    io::{Cursor, Read, Seek},
};
use zip::ZipArchive;

// ─────────────────────────────────────────────────────────────
// 对外类型
// ─────────────────────────────────────────────────────────────

// #[derive(Debug, Clone)]
// pub enum CellValue {
//     Empty,
//     Str(Arc<str>),
//     Num(f64),
// }

// impl CellValue {
//     #[inline]
//     pub fn is_empty_val(&self) -> bool {
//         matches!(self, CellValue::Empty) || matches!(self, CellValue::Str(s) if s.trim().is_empty())
//     }

//     #[inline]
//     pub fn as_str(&self) -> Option<&str> {
//         match self {
//             CellValue::Str(s) => Some(s.as_ref()),
//             _ => None,
//         }
//     }

//     #[inline]
//     pub fn as_f64(&self) -> Option<f64> {
//         match self {
//             CellValue::Num(n) => Some(*n),
//             CellValue::Str(s) => s.trim().parse().ok(),
//             _ => None,
//         }
//     }

//     #[inline]
//     pub fn to_string_opt(&self) -> Option<String> {
//         match self {
//             CellValue::Str(s) if !s.trim().is_empty() => Some(s.to_string()),
//             CellValue::Num(n) => Some(n.to_string()),
//             _ => None,
//         }
//     }

//     #[inline]
//     pub fn to_display(&self) -> String {
//         match self {
//             CellValue::Empty => String::new(),
//             CellValue::Str(s) => s.to_string(),
//             CellValue::Num(n) => n.to_string(),
//         }
//     }
// }
/* #[derive(Debug, Clone)]
pub enum CellValue {
    Empty,
    StrRef(usize),     // ✅ 指向 sharedStrings
    InlineStr(String), // ✅ 直接字符串
    Num(f64),
} */
#[derive(Debug, Clone)]
pub enum CellValue {
    Empty,
    StrRef(usize),
    InlineStr(String),
    Raw(String),
}

impl CellValue {
    #[inline]
    pub fn is_empty_val(&self) -> bool {
        matches!(self, CellValue::Empty)
            || matches!(self, CellValue::InlineStr(s) if s.trim().is_empty())
            || matches!(self, CellValue::Raw(s) if s.trim().is_empty())
    }

    #[inline]
    pub fn as_str<'a>(&'a self, shared: &'a [String]) -> Option<&'a str> {
        match self {
            CellValue::StrRef(i) => shared.get(*i).map(|s| s.as_str()),
            CellValue::InlineStr(s) => Some(s.as_str()),
            CellValue::Raw(s) => Some(s.as_str()),
            CellValue::Empty => None,
        }
    }

    #[inline]
    pub fn as_f64(&self, shared: &[String]) -> Option<f64> {
        self.as_str(shared)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .and_then(|s| s.replace(',', "").parse::<f64>().ok())
    }

    #[inline]
    pub fn to_display(&self, shared: &[String]) -> String {
        self.as_str(shared).unwrap_or("").to_string()
    }
}

pub type SheetRow = Vec<CellValue>;

#[derive(Debug, Clone)]
pub struct SheetData {
    pub rows: Vec<SheetRow>,
}

impl SheetData {
    #[inline]
    pub fn data_rows(&self) -> &[SheetRow] {
        if self.rows.len() <= 1 {
            &[]
        } else {
            &self.rows[1..]
        }
    }
}

// ─────────────────────────────────────────────────────────────
// 内部类型
// ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct SheetEntry {
    name: String,
    /// 例: xl/worksheets/sheet1.xml
    path: String,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum CellType {
    Number,
    SharedString,
    InlineStr,
    Str,
    Bool,
}

// ─────────────────────────────────────────────────────────────
// 主入口
// ─────────────────────────────────────────────────────────────

/// 读取 workbook 中所有 sheet

pub fn read_xlsx(path: &str) -> Result<(HashMap<String, SheetData>, Vec<String>), AppError> {
    read_xlsx_selected(path, &[])
}

/// 只读取指定 sheet；当 wanted 为空时，读取全部 sheet
pub fn read_xlsx_selected(
    path: &str,
    wanted: &[&str],
) -> Result<(HashMap<String, SheetData>, Vec<String>), AppError> {
    let file = std::fs::File::open(path).map_err(AppError::Io)?;
    let mut zip = ZipArchive::new(file).map_err(|e| AppError::ExcelParse(e.to_string()))?;

    // Step 1: 读取必要元数据（单线程）
    let workbook_bytes = read_zip_entry(&mut zip, "xl/workbook.xml")
        .map_err(|e| AppError::ExcelParse(e.to_string()))?;
    let rel_bytes = read_zip_entry(&mut zip, "xl/_rels/workbook.xml.rels")
        .map_err(|e| AppError::ExcelParse(e.to_string()))?;
    let shared_bytes = read_optional_zip_entry(&mut zip, "xl/sharedStrings.xml")
        .map_err(|e| AppError::ExcelParse(e.to_string()))?;

    // Step 2: 解析 sheet 列表
    let mut sheet_entries = parse_workbook_and_relationships(&workbook_bytes, &rel_bytes)
        .map_err(|e| AppError::ExcelParse(e.to_string()))?;

    if !wanted.is_empty() {
        let wanted_set: HashSet<&str> = wanted.iter().copied().collect();
        sheet_entries.retain(|e| wanted_set.contains(e.name.as_str()));
    }

    // Step 3: 顺序读取所有目标 sheet XML bytes（避免 ZipArchive 并发）
    let mut sheet_bytes: Vec<(String, Vec<u8>)> = Vec::with_capacity(sheet_entries.len());
    for entry in &sheet_entries {
        let bytes = read_zip_entry(&mut zip, &entry.path)
            .map_err(|e| AppError::ExcelParse(e.to_string()))?;
        sheet_bytes.push((entry.name.clone(), bytes));
    }

    // Step 4: sharedStrings 只解析一次
    // let shared: Arc<Vec<Arc<str>>> = Arc::new(match shared_bytes {
    //     Some(bytes) => {
    //         parse_shared_strings(&bytes).map_err(|e| AppError::ExcelParse(e.to_string()))?
    //     }
    //     None => Vec::new(),
    // });

    let shared: Vec<String> = match shared_bytes {
        Some(bytes) => {
            parse_shared_strings(&bytes).map_err(|e| AppError::ExcelParse(e.to_string()))?
        }
        None => Vec::new(),
    };

    // Step 5: rayon 并行解析 sheet XML
    let results: Vec<Result<(String, SheetData), AppError>> = sheet_bytes
        .into_par_iter()
        .map(|(name, bytes)| {
            // let data = parse_sheet_xml(&bytes, &shared)
            let data = parse_sheet_xml(&bytes).map_err(|e| AppError::ExcelParse(e.to_string()))?;
            Ok((name, data))
        })
        .collect();

    // Step 6: 收集
    let mut map = HashMap::with_capacity(results.len());
    for item in results {
        let (name, data) = item?;
        map.insert(name, data);
    }

    Ok((map, shared))
}

// ─────────────────────────────────────────────────────────────
// Zip 读取
// ─────────────────────────────────────────────────────────────

fn read_zip_entry<R: Read + Seek>(zip: &mut ZipArchive<R>, name: &str) -> Result<Vec<u8>> {
    let mut entry = zip
        .by_name(name)
        .with_context(|| format!("missing zip entry: {name}"))?;

    let mut bytes = Vec::with_capacity(entry.size() as usize);
    entry
        .read_to_end(&mut bytes)
        .with_context(|| format!("failed to read zip entry: {name}"))?;
    Ok(bytes)
}

fn read_optional_zip_entry<R: Read + Seek>(
    zip: &mut ZipArchive<R>,
    name: &str,
) -> Result<Option<Vec<u8>>> {
    match zip.by_name(name) {
        Ok(mut entry) => {
            let mut bytes = Vec::with_capacity(entry.size() as usize);
            entry
                .read_to_end(&mut bytes)
                .with_context(|| format!("failed to read zip entry: {name}"))?;
            Ok(Some(bytes))
        }
        Err(zip::result::ZipError::FileNotFound) => Ok(None),
        Err(err) => {
            Err(anyhow!(err)).with_context(|| format!("failed to access zip entry: {name}"))
        }
    }
}

// ─────────────────────────────────────────────────────────────
// workbook / rels 解析
// ─────────────────────────────────────────────────────────────

fn parse_workbook_and_relationships(
    workbook_xml: &[u8],
    rel_xml: &[u8],
) -> Result<Vec<SheetEntry>> {
    let rel_map = parse_relationships(rel_xml)?;

    let mut reader = Reader::from_reader(Cursor::new(workbook_xml));
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut sheets = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) if e.name().as_ref() == b"sheet" => {
                let mut sheet_name: Option<String> = None;
                let mut rel_id: Option<String> = None;

                for attr in e.attributes() {
                    let attr = attr.context("failed to read workbook sheet attribute")?;
                    match attr.key.as_ref() {
                        b"name" => {
                            let v = attr
                                .decode_and_unescape_value(e.decoder())
                                .context("failed to decode workbook sheet name")?;
                            sheet_name = Some(v.into_owned());
                        }
                        b"r:id" => {
                            let v = attr
                                .decode_and_unescape_value(e.decoder())
                                .context("failed to decode workbook sheet r:id")?;
                            rel_id = Some(v.into_owned());
                        }
                        _ => {}
                    }
                }

                let name = sheet_name.unwrap_or_else(|| "unknown".to_string());
                let rid = rel_id.ok_or_else(|| anyhow!("missing r:id for sheet '{name}'"))?;
                let target = rel_map
                    .get(&rid)
                    .ok_or_else(|| anyhow!("no relationship target for r:id '{rid}'"))?;

                sheets.push(SheetEntry {
                    name,
                    path: normalize_zip_path(target),
                });
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(err) => return Err(anyhow!(err)).context("failed to parse workbook.xml"),
        }
        buf.clear();
    }

    Ok(sheets)
}

fn parse_relationships(xml: &[u8]) -> Result<HashMap<String, String>> {
    let mut reader = Reader::from_reader(Cursor::new(xml));
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut map = HashMap::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e))
                if e.name().as_ref().ends_with(b"Relationship") =>
            {
                let mut id: Option<String> = None;
                let mut target: Option<String> = None;

                for attr in e.attributes() {
                    let attr = attr.context("failed to read relationship attribute")?;
                    match attr.key.as_ref() {
                        b"Id" => {
                            let v = attr
                                .decode_and_unescape_value(e.decoder())
                                .context("failed to decode relationship Id")?;
                            id = Some(v.into_owned());
                        }
                        b"Target" => {
                            let v = attr
                                .decode_and_unescape_value(e.decoder())
                                .context("failed to decode relationship Target")?;
                            target = Some(v.into_owned());
                        }
                        _ => {}
                    }
                }

                if let (Some(id), Some(target)) = (id, target) {
                    map.insert(id, target);
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(err) => return Err(anyhow!(err)).context("failed to parse workbook.xml.rels"),
        }
        buf.clear();
    }

    Ok(map)
}

fn parse_shared_strings(xml: &[u8]) -> Result<Vec<String>> {
    let mut reader = Reader::from_reader(Cursor::new(xml));
    reader.config_mut().trim_text(false);

    let mut buf = Vec::new();
    let mut shared_strings: Vec<String> = Vec::new();

    let mut current = String::new();
    let mut in_si = false;
    let mut in_t = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) if e.name().as_ref() == b"si" => {
                in_si = true;
                current.clear();
            }
            Ok(Event::End(e)) if e.name().as_ref() == b"si" => {
                if in_si {
                    shared_strings.push(std::mem::take(&mut current));
                    in_si = false;
                }
            }
            Ok(Event::Start(e)) if e.name().as_ref() == b"t" => {
                in_t = true;
            }
            Ok(Event::End(e)) if e.name().as_ref() == b"t" => {
                in_t = false;
            }
            Ok(Event::Text(e)) if in_si && in_t => {
                current.push_str(&e.xml_content()?);
            }
            Ok(Event::CData(e)) if in_si && in_t => {
                current.push_str(&e.xml_content()?);
            }
            Ok(Event::GeneralRef(r)) if in_si && in_t => {
                if let Some(ch) = r.resolve_char_ref()? {
                    current.push(ch);
                } else {
                    let text = r.decode()?;
                    current.push('&');
                    current.push_str(&text);
                    current.push(';');
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(e) => return Err(anyhow!(e)),
        }
        buf.clear();
    }

    Ok(shared_strings)
}
// ─────────────────────────────────────────────────────────────
// sheet XML 解析
// ─────────────────────────────────────────────────────────────

fn parse_sheet_xml(bytes: &[u8]) -> Result<SheetData> {
    let mut reader = Reader::from_reader(Cursor::new(bytes));
    reader.config_mut().trim_text(false);

    let mut buf = Vec::new();

    let mut rows: Vec<SheetRow> = Vec::new();
    let mut cur_row: SheetRow = Vec::with_capacity(32);
    let mut scratch = String::new();

    let mut cur_col_idx: usize = usize::MAX;
    let mut cur_cell_type = CellType::Number;
    let mut in_row = false;
    let mut in_cell = false;
    let mut in_value = false;
    let mut cell_had_value = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) if e.name().as_ref() == b"row" => {
                cur_row.clear();
                in_row = true;
            }

            Ok(Event::Start(ref e)) if e.name().as_ref() == b"c" => {
                scratch.clear();
                cell_had_value = false;
                in_cell = true;

                let (col_idx, cell_type) = parse_cell_attrs(e);
                cur_col_idx = col_idx;
                cur_cell_type = cell_type;

                if cur_col_idx != usize::MAX {
                    while cur_row.len() < cur_col_idx {
                        cur_row.push(CellValue::Empty);
                    }
                }
            }

            Ok(Event::Empty(ref e)) if e.name().as_ref() == b"c" => {
                let (col_idx, _) = parse_cell_attrs(e);

                if col_idx != usize::MAX {
                    while cur_row.len() < col_idx {
                        cur_row.push(CellValue::Empty);
                    }
                }

                cur_row.push(CellValue::Empty);
            }

            Ok(Event::Start(ref e)) if in_cell && matches!(e.name().as_ref(), b"v" | b"t") => {
                in_value = true;
                scratch.clear();
            }

            Ok(Event::Text(ref text)) if in_value => {
                scratch.push_str(
                    &text
                        .xml_content()
                        .context("failed to decode sheet cell text")?,
                );
            }

            Ok(Event::CData(ref text)) if in_value => {
                scratch.push_str(
                    &text
                        .xml_content()
                        .context("failed to decode sheet cell cdata")?,
                );
            }

            // xlsx 常用数字字符引用（如 &#38144;）转义中文，quick-xml 0.38+
            // 把它们拆成独立的 GeneralRef 事件，不会包含在 Text 事件里，
            // 必须单独解析并拼回 scratch，否则中文内容会被整体丢弃/截断。
            Ok(Event::GeneralRef(ref r)) if in_value => {
                if let Some(ch) = r.resolve_char_ref().context("failed to resolve char ref")? {
                    scratch.push(ch);
                } else {
                    let text = r
                        .decode()
                        .context("failed to decode general entity reference")?;
                    scratch.push('&');
                    scratch.push_str(&text);
                    scratch.push(';');
                }
            }

            Ok(Event::End(ref e)) if in_value && matches!(e.name().as_ref(), b"v" | b"t") => {
                in_value = false;
                cell_had_value = true;

                let value = resolve_cell_value(&scratch, cur_cell_type);
                cur_row.push(value);
            }

            Ok(Event::End(ref e)) if e.name().as_ref() == b"c" => {
                if !cell_had_value {
                    if cur_col_idx != usize::MAX {
                        if cur_row.len() == cur_col_idx {
                            cur_row.push(CellValue::Empty);
                        }
                    } else {
                        cur_row.push(CellValue::Empty);
                    }
                }

                cur_col_idx = usize::MAX;
                cur_cell_type = CellType::Number;
                cell_had_value = false;
                in_cell = false;
                in_value = false;
            }

            Ok(Event::End(ref e)) if e.name().as_ref() == b"row" => {
                if in_row {
                    rows.push(std::mem::replace(&mut cur_row, Vec::with_capacity(32)));
                    in_row = false;
                }
            }

            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(err) => return Err(anyhow!(err)).context("failed to parse worksheet xml"),
        }

        buf.clear();
    }

    Ok(SheetData { rows })
}

#[inline]
fn resolve_cell_value(scratch: &str, cell_type: CellType) -> CellValue {
    match cell_type {
        CellType::SharedString => match scratch.trim().parse::<usize>() {
            Ok(i) => CellValue::StrRef(i),
            Err(_) => CellValue::Empty,
        },

        CellType::InlineStr => {
            if scratch.is_empty() {
                CellValue::Empty
            } else {
                CellValue::InlineStr(scratch.to_string())
            }
        }

        CellType::Str | CellType::Bool | CellType::Number => {
            let s = scratch.trim();

            if s.is_empty() {
                CellValue::Empty
            } else {
                CellValue::Raw(s.to_string())
            }
        }
    }
}

#[inline]
fn parse_cell_attrs(e: &BytesStart<'_>) -> (usize, CellType) {
    let mut col_idx = usize::MAX;
    let mut cell_type = CellType::Number;

    for attr in e.attributes().flatten() {
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

    (col_idx, cell_type)
}

// ─────────────────────────────────────────────────────────────
// 工具函数
// ─────────────────────────────────────────────────────────────

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
fn normalize_zip_path(target: &str) -> String {
    let t = target.trim_start_matches('/');

    if t.starts_with("xl/") {
        t.to_string()
    } else {
        format!("xl/{t}")
    }
}

// ─────────────────────────────────────────────────────────────
// Excel 日期处理（保留）
// ─────────────────────────────────────────────────────────────

pub fn excel_serial_to_datetime(serial: f64) -> Option<chrono::NaiveDateTime> {
    use chrono::{Duration, NaiveDate};

    let epoch = NaiveDate::from_ymd_opt(1899, 12, 30)?.and_hms_opt(0, 0, 0)?;
    let days = serial.floor() as i64;
    let frac = serial.fract();
    let secs = (frac * 86400.0).round() as i64;

    Some(epoch + Duration::days(days) + Duration::seconds(secs))
}

pub fn excel_serial_to_date_string(serial: f64) -> String {
    excel_serial_to_datetime(serial)
        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_default()
}
