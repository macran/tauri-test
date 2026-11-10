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

// use std::collections::HashMap;
// use std::io::{Cursor, Read, Seek};
// use std::sync::Arc;

// use quick_xml::events::{BytesStart, Event};
// use quick_xml::Reader;
// use zip::ZipArchive;

// use crate::error::AppError;

// // ─────────────────────────────────────────────────────────────
// // 公开类型
// // ─────────────────────────────────────────────────────────────

// #[derive(Debug, Clone, PartialEq)]
// pub enum CellValue {
//     Empty,
//     String(String),
//     Number(f64),
//     Bool(bool),
// }
// impl std::fmt::Display for CellValue {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         match self {
//             CellValue::Empty => write!(f, ""),
//             CellValue::String(s) => write!(f, "{s}"),
//             CellValue::Number(n) => write!(f, "{n}"),
//             CellValue::Bool(b) => write!(f, "{b}"),
//         }
//     }
// }

// impl CellValue {
//     /// 取字符串引用（仅 String 变体有效）
//     pub fn as_str(&self) -> Option<&str> {
//         match self {
//             CellValue::String(s) => Some(s.as_str()),
//             _ => None,
//         }
//     }

//     /// 取 f64（Number 直接返回，String 尝试 parse）
//     pub fn as_f64(&self) -> Option<f64> {
//         match self {
//             CellValue::Number(n) => Some(*n),
//             CellValue::String(s) => s.parse::<f64>().ok(),
//             _ => None,
//         }
//     }

//     /// 是否为空（Empty 或空白字符串）
//     pub fn is_empty_val(&self) -> bool {
//         match self {
//             CellValue::Empty => true,
//             CellValue::String(s) => s.trim().is_empty(),
//             _ => false,
//         }
//     }

//     /// 取 String（克隆）
//     pub fn to_string_opt(&self) -> Option<String> {
//         match self {
//             CellValue::String(s) if !s.trim().is_empty() => Some(s.clone()),
//             CellValue::Number(n) => Some(n.to_string()),
//             _ => None,
//         }
//     }

//     pub fn to_display(&self) -> String {
//         match self {
//             CellValue::Empty => String::new(),
//             CellValue::Str(s) => s.clone(),
//             CellValue::Num(n) => n.to_string(),
//         }
//     }
// }

// #[derive(Debug, Clone)]
// pub struct SheetRow {
//     pub cells: Vec<CellValue>,
// }

// impl SheetRow {
//     pub fn get_string(&self, col: usize) -> String {
//         match self.cells.get(col) {
//             Some(v) => v.to_string(),
//             None => String::new(),
//         }
//     }

//     pub fn get_f64(&self, col: usize) -> Option<f64> {
//         match self.cells.get(col) {
//             Some(CellValue::Number(n)) => Some(*n),
//             Some(CellValue::String(s)) => s.parse().ok(),
//             _ => None,
//         }
//     }

//     pub fn len(&self) -> usize {
//         self.cells.len()
//     }

//     pub fn is_empty_row(&self) -> bool {
//         self.cells.iter().all(|c| c.is_empty())
//     }
// }

// pub struct SheetData {
//     /// rows[0] = 表头行，rows[1..] = 数据行
//     pub rows: Vec<Vec<CellValue>>,
// }

// impl SheetData {
//     pub fn data_rows(&self) -> &[Vec<CellValue>] {
//         if self.rows.is_empty() {
//             &[]
//         } else {
//             &self.rows[1..]
//         }
//     }
// }
// // ─────────────────────────────────────────────────────────────
// // 主入口
// // ─────────────────────────────────────────────────────────────

// pub fn read_xlsx_sheets(
//     file_path: &str,
//     sheet_names: &[&str],
// ) -> Result<HashMap<String, SheetData>, AppError> {
//     let file_bytes = std::fs::read(file_path).map_err(AppError::Io)?;
//     read_xlsx_from_bytes(&file_bytes, sheet_names)
// }

// pub fn read_xlsx_from_bytes(
//     file_bytes: &[u8],
//     sheet_names: &[&str],
// ) -> Result<HashMap<String, SheetData>, AppError> {
//     let cursor = Cursor::new(file_bytes);
//     let mut zip =
//         ZipArchive::new(cursor).map_err(|e| AppError::ExcelParse(format!("无法打开 zip: {e}")))?;

//     // Step 1: 解析 workbook.xml + rels，建立 sheet名 → zip路径 映射
//     let workbook_xml = read_zip_entry(&mut zip, "xl/workbook.xml")?;
//     let rel_xml = read_zip_entry(&mut zip, "xl/_rels/workbook.xml.rels")?;
//     let sheet_path_map = parse_sheet_paths(&workbook_xml, &rel_xml)?;

//     // Step 2: 解析 sharedStrings（只解析一次！）
//     let shared_strings = match read_optional_zip_entry(&mut zip, "xl/sharedStrings.xml")? {
//         Some(xml) => parse_shared_strings(&xml)?,
//         None => Vec::new(),
//     };
//     let shared_strings = Arc::new(shared_strings);

//     // Step 3: 顺序读取需要的 sheet XML bytes（单线程顺序读 zip，避免 seek 竞争）
//     let mut sheet_xmls: Vec<(String, Vec<u8>)> = Vec::new();
//     for name in sheet_names {
//         if let Some(path) = sheet_path_map.get(*name) {
//             if let Some(xml) = read_optional_zip_entry(&mut zip, path)? {
//                 sheet_xmls.push((name.to_string(), xml));
//             }
//         }
//         // sheet 不存在则跳过（可选 sheet 正常）
//     }

//     // Step 4: rayon 并行解析各 sheet XML（纯 CPU，无 I/O 竞争）
//     use rayon::prelude::*;
//     let results: Vec<Result<SheetData, AppError>> = sheet_xmls
//         .into_par_iter()
//         .map(|(name, xml)| {
//             let ss = Arc::clone(&shared_strings);
//             let rows = parse_sheet_xml(&xml, &ss)?;
//             Ok(SheetData { name, rows })
//         })
//         .collect();

//     let mut map = HashMap::new();
//     for result in results {
//         let sheet = result?;
//         map.insert(sheet.name.clone(), sheet);
//     }
//     Ok(map)
// }

// // ─────────────────────────────────────────────────────────────
// // 内部解析函数 — 完全照抄用户提供的测试通过的逻辑，只把错误处理改为 AppError
// // ─────────────────────────────────────────────────────────────

// fn parse_sheet_paths(
//     workbook_xml: &[u8],
//     rel_xml: &[u8],
// ) -> Result<HashMap<String, String>, AppError> {
//     let rel_map = parse_relationships(rel_xml)?;
//     let mut reader = Reader::from_reader(Cursor::new(workbook_xml));
//     reader.config_mut().trim_text(true);

//     let mut buf = Vec::new();
//     let mut map = HashMap::new();

//     loop {
//         match reader.read_event_into(&mut buf) {
//             Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) if e.name().as_ref() == b"sheet" => {
//                 let name = get_attr(e, b"name")?.unwrap_or_else(|| "unknown".to_string());
//                 let rel_id = get_attr(e, b"r:id")?
//                     .ok_or_else(|| AppError::ExcelParse(format!("sheet {name} 缺少 r:id")))?;
//                 if let Some(target) = rel_map.get(&rel_id) {
//                     map.insert(name, normalize_zip_path(target));
//                 }
//             }
//             Ok(Event::Eof) => break,
//             Ok(_) => {}
//             Err(e) => {
//                 return Err(AppError::ExcelParse(format!("解析 workbook.xml 失败: {e}")));
//             }
//         }
//         buf.clear();
//     }
//     Ok(map)
// }

// fn parse_relationships(xml: &[u8]) -> Result<HashMap<String, String>, AppError> {
//     let mut reader = Reader::from_reader(Cursor::new(xml));
//     reader.config_mut().trim_text(true);

//     let mut buf = Vec::new();
//     let mut map = HashMap::new();

//     loop {
//         match reader.read_event_into(&mut buf) {
//             Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e))
//                 if e.name().as_ref().ends_with(b"Relationship") =>
//             {
//                 let id = get_attr(e, b"Id")?;
//                 let target = get_attr(e, b"Target")?;
//                 if let (Some(id), Some(target)) = (id, target) {
//                     map.insert(id, target);
//                 }
//             }
//             Ok(Event::Eof) => break,
//             Ok(_) => {}
//             Err(e) => {
//                 return Err(AppError::ExcelParse(format!("解析 rels 失败: {e}")));
//             }
//         }
//         buf.clear();
//     }
//     Ok(map)
// }

// /// 完全照抄用户原始代码逻辑，只把错误类型改为 AppError
// fn parse_shared_strings(xml: &[u8]) -> Result<Vec<String>, AppError> {
//     let mut reader = Reader::from_reader(Cursor::new(xml));
//     reader.config_mut().trim_text(false);

//     let mut buf = Vec::new();
//     let mut shared_strings = Vec::new();
//     let mut current_text = String::new();
//     let mut inside_si = false;
//     let mut inside_text = false;

//     loop {
//         match reader.read_event_into(&mut buf) {
//             Ok(Event::Start(ref e)) if e.name().as_ref() == b"si" => {
//                 inside_si = true;
//                 current_text.clear();
//             }
//             Ok(Event::End(ref e)) if e.name().as_ref() == b"si" => {
//                 if inside_si {
//                     shared_strings.push(current_text.clone());
//                     current_text.clear();
//                     inside_si = false;
//                 }
//             }
//             Ok(Event::Start(ref e)) if e.name().as_ref() == b"t" => {
//                 inside_text = true;
//             }
//             Ok(Event::End(ref e)) if e.name().as_ref() == b"t" => {
//                 inside_text = false;
//             }
//             Ok(Event::Text(text)) => {
//                 if inside_si && inside_text {
//                     current_text.push_str(&text.xml_content().map_err(|e| {
//                         AppError::ExcelParse(format!("decode sharedString text: {e}"))
//                     })?);
//                 }
//             }
//             Ok(Event::CData(text)) => {
//                 if inside_si && inside_text {
//                     current_text.push_str(&text.xml_content().map_err(|e| {
//                         AppError::ExcelParse(format!("decode sharedString cdata: {e}"))
//                     })?);
//                 }
//             }
//             Ok(Event::Eof) => break,
//             Ok(_) => {}
//             Err(e) => {
//                 return Err(AppError::ExcelParse(format!(
//                     "解析 sharedStrings.xml 失败: {e}"
//                 )));
//             }
//         }
//         buf.clear();
//     }
//     Ok(shared_strings)
// }

// /// 完全照抄用户原始代码逻辑，只把错误类型改为 AppError
// fn parse_sheet_xml(xml: &[u8], shared_strings: &[String]) -> Result<Vec<SheetRow>, AppError> {
//     let mut reader = Reader::from_reader(Cursor::new(xml));
//     reader.config_mut().trim_text(false);

//     let mut buf = Vec::new();
//     let mut rows: Vec<SheetRow> = Vec::new();
//     let mut current_row: Option<Vec<(usize, CellValue)>> = None;
//     let mut current_col_idx: usize = 0;
//     let mut current_cell_type: Option<String> = None; // 原始代码类型，保持不变
//     let mut inside_value = false;
//     let mut scratch = String::new();

//     loop {
//         match reader.read_event_into(&mut buf) {
//             Ok(Event::Start(ref e)) if e.name().as_ref() == b"row" => {
//                 current_row = Some(Vec::new());
//             }
//             Ok(Event::End(ref e)) if e.name().as_ref() == b"row" => {
//                 if let Some(cells_sparse) = current_row.take() {
//                     let max_col = cells_sparse
//                         .iter()
//                         .map(|(i, _)| *i)
//                         .max()
//                         .map(|m| m + 1)
//                         .unwrap_or(0);
//                     let mut dense = vec![CellValue::Empty; max_col];
//                     for (idx, val) in cells_sparse {
//                         dense[idx] = val;
//                     }
//                     rows.push(SheetRow { cells: dense });
//                 }
//             }
//             Ok(Event::Start(ref e)) if e.name().as_ref() == b"c" => {
//                 current_col_idx = get_attr(e, b"r")?
//                     .as_deref()
//                     .map(col_index_from_ref)
//                     .unwrap_or(0);
//                 current_cell_type = get_attr(e, b"t")?;
//             }
//             Ok(Event::Empty(ref e)) if e.name().as_ref() == b"c" => {
//                 let col = get_attr(e, b"r")?
//                     .as_deref()
//                     .map(col_index_from_ref)
//                     .unwrap_or(0);
//                 if let Some(ref mut row) = current_row {
//                     row.push((col, CellValue::Empty));
//                 }
//             }
//             Ok(Event::Start(ref e)) if e.name().as_ref() == b"v" || e.name().as_ref() == b"is" => {
//                 inside_value = true;
//                 scratch.clear();
//             }
//             Ok(Event::End(ref e)) if e.name().as_ref() == b"v" || e.name().as_ref() == b"is" => {
//                 // 原始代码的 match 逻辑，完全保留
//                 let value = match current_cell_type.as_deref() {
//                     Some("s") => {
//                         if let Ok(index) = scratch.parse::<usize>() {
//                             shared_strings
//                                 .get(index)
//                                 .map(|s| CellValue::String(s.clone()))
//                                 .unwrap_or(CellValue::Empty)
//                         } else {
//                             CellValue::Empty
//                         }
//                     }
//                     Some("b") => CellValue::Bool(scratch.trim() == "1"),
//                     Some("str") | Some("e") => CellValue::String(scratch.clone()),
//                     _ => {
//                         if scratch.is_empty() {
//                             CellValue::Empty
//                         } else {
//                             scratch
//                                 .parse::<f64>()
//                                 .map(CellValue::Number)
//                                 .unwrap_or_else(|_| CellValue::String(scratch.clone()))
//                         }
//                     }
//                 };
//                 if let Some(ref mut row) = current_row {
//                     row.push((current_col_idx, value));
//                 }
//                 inside_value = false;
//                 scratch.clear();
//                 current_cell_type = None;
//             }
//             Ok(Event::Text(text)) => {
//                 if inside_value {
//                     scratch.push_str(
//                         &text
//                             .xml_content()
//                             .map_err(|e| AppError::ExcelParse(format!("decode sheet text: {e}")))?,
//                     );
//                 }
//             }
//             Ok(Event::CData(text)) => {
//                 if inside_value {
//                     scratch.push_str(
//                         &text.xml_content().map_err(|e| {
//                             AppError::ExcelParse(format!("decode sheet cdata: {e}"))
//                         })?,
//                     );
//                 }
//             }
//             Ok(Event::Eof) => break,
//             Ok(_) => {}
//             Err(e) => {
//                 return Err(AppError::ExcelParse(format!("解析 sheet XML 失败: {e}")));
//             }
//         }
//         buf.clear();
//     }
//     Ok(rows)
// }

// // ─────────────────────────────────────────────────────────────
// // 工具函数
// // ─────────────────────────────────────────────────────────────

// /// Excel 列地址（如 "A1", "BC42"）转 0-based 列索引
// fn col_index_from_ref(cell_ref: &str) -> usize {
//     let mut col = 0usize;
//     for ch in cell_ref.bytes() {
//         if ch.is_ascii_alphabetic() {
//             col = col * 26 + (ch.to_ascii_uppercase() - b'A') as usize + 1;
//         } else {
//             break;
//         }
//     }
//     col.saturating_sub(1)
// }

// fn get_attr(element: &BytesStart<'_>, key: &[u8]) -> Result<Option<String>, AppError> {
//     for attr in element.attributes() {
//         let attr = attr.map_err(|e| AppError::ExcelParse(format!("读取 xml 属性失败: {e}")))?;
//         if attr.key.as_ref() == key {
//             return attr
//                 .decode_and_unescape_value(element.decoder())
//                 .map(|v| Some(v.into_owned()))
//                 .map_err(|e| AppError::ExcelParse(format!("解码 xml 属性失败: {e}")));
//         }
//     }
//     Ok(None)
// }

// fn normalize_zip_path(target: &str) -> String {
//     if target.starts_with("xl/") {
//         target.to_string()
//     } else {
//         format!("xl/{target}")
//     }
// }

// fn read_zip_entry<R: Read + Seek>(
//     zip: &mut ZipArchive<R>,
//     name: &str,
// ) -> Result<Vec<u8>, AppError> {
//     let mut entry = zip
//         .by_name(name)
//         .map_err(|e| AppError::ExcelParse(format!("zip 条目 {name} 不存在: {e}")))?;
//     let mut bytes = Vec::with_capacity(entry.size() as usize);
//     entry
//         .read_to_end(&mut bytes)
//         .map_err(|e| AppError::ExcelParse(format!("读取 zip 条目 {name} 失败: {e}")))?;
//     Ok(bytes)
// }

// fn read_optional_zip_entry<R: Read + Seek>(
//     zip: &mut ZipArchive<R>,
//     name: &str,
// ) -> Result<Option<Vec<u8>>, AppError> {
//     match zip.by_name(name) {
//         Ok(mut entry) => {
//             let mut bytes = Vec::with_capacity(entry.size() as usize);
//             entry
//                 .read_to_end(&mut bytes)
//                 .map_err(|e| AppError::ExcelParse(format!("读取 zip 条目 {name} 失败: {e}")))?;
//             Ok(Some(bytes))
//         }
//         Err(zip::result::ZipError::FileNotFound) => Ok(None),
//         Err(e) => Err(AppError::ExcelParse(format!(
//             "访问 zip 条目 {name} 失败: {e}"
//         ))),
//     }
// }

// // ─────────────────────────────────────────────────────────────
// // 日期处理
// // ─────────────────────────────────────────────────────────────

// /// Excel 日期序列号 → chrono NaiveDateTime
// /// Excel epoch: 1899-12-30（Lotus 兼容 bug）
// pub fn excel_serial_to_datetime(serial: f64) -> Option<chrono::NaiveDateTime> {
//     use chrono::{Duration, NaiveDate};
//     let epoch = NaiveDate::from_ymd_opt(1899, 12, 30)?.and_hms_opt(0, 0, 0)?;
//     let days = serial.floor() as i64;
//     let frac = serial.fract();
//     let secs = (frac * 86400.0).round() as i64;
//     Some(epoch + Duration::days(days) + Duration::seconds(secs))
// }

// pub fn excel_serial_to_date_string(serial: f64) -> String {
//     excel_serial_to_datetime(serial)
//         .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
//         .unwrap_or_default()
// }
// src/excel_reader.rs

//! excel_reader.rs
//!
//! 读取 .xlsx 文件，返回 sheet_name -> SheetData 的 HashMap。
//!
//! 设计要点：
//!   1. 主线程顺序读完所有 zip entry（ZipArchive 不是 Send）
//!   2. 各 sheet xml 字节交给 rayon 并行解析
//!   3. 所有 XML 属性通过 get_attr + element.decoder() 提取
//!   4. 文本内容通过 BytesText::xml_content() 提取（处理 XML 实体）

// use crate::error::AppError;
// use anyhow::{anyhow, Context, Result};
// use quick_xml::{
//     events::{BytesStart, Event},
//     Reader,
// };
// use rayon::prelude::*;
// use std::{
//     collections::HashMap,
//     io::{Cursor, Read, Seek},
//     sync::Arc,
// };
// use zip::ZipArchive;

// // ── 对外类型 ──────────────────────────────────────────────────

// #[derive(Debug, Clone)]
// pub enum CellValue {
//     Empty,
//     Str(String), // 字符串（含 sharedString）
//     Num(f64),    // 数字
// }

// impl CellValue {
//     pub fn is_empty_val(&self) -> bool {
//         matches!(self, CellValue::Empty) || matches!(self, CellValue::Str(s) if s.trim().is_empty())
//     }

//     pub fn as_str(&self) -> Option<&str> {
//         match self {
//             CellValue::Str(s) => Some(s.as_str()),
//             _ => None,
//         }
//     }

//     pub fn as_f64(&self) -> Option<f64> {
//         match self {
//             CellValue::Num(n) => Some(*n),
//             // 容错：有时数字列会被解析为 Str（如内联字符串单元格）
//             CellValue::Str(s) => s.trim().parse().ok(),
//             _ => None,
//         }
//     }

//     /// 取 String（克隆），空字符串返回 None
//     pub fn to_string_opt(&self) -> Option<String> {
//         match self {
//             CellValue::Str(s) if !s.trim().is_empty() => Some(s.clone()),
//             CellValue::Num(n) => Some(n.to_string()),
//             _ => None,
//         }
//     }

//     pub fn to_display(&self) -> String {
//         match self {
//             CellValue::Empty => String::new(),
//             CellValue::Str(s) => s.clone(),
//             CellValue::Num(n) => n.to_string(),
//         }
//     }
// }

// pub type SheetRow = Vec<CellValue>;

// #[derive(Debug, Clone)]
// pub struct SheetData {
//     pub rows: Vec<SheetRow>,
// }

// impl SheetData {
//     /// 跳过第一行（表头），返回数据行切片
//     pub fn data_rows(&self) -> &[SheetRow] {
//         if self.rows.len() <= 1 {
//             &[]
//         } else {
//             &self.rows[1..]
//         }
//     }
// }

// // ── 内部辅助结构体 ───────────────────────────────────────────

// struct SheetEntry {
//     name: String,
//     /// 已标准化的 zip 路径，如 "xl/worksheets/sheet1.xml"
//     path: String,
// }

// // ── 主入口 ────────────────────────────────────────────────────

// /// 读取 xlsx，返回 sheet_name -> SheetData 的 HashMap。
// /// 内部用 rayon 并行解析各 sheet。
// pub fn read_xlsx(path: &str) -> Result<HashMap<String, SheetData>, AppError> {
//     let file = std::fs::File::open(path).map_err(AppError::Io)?;
//     let mut zip = ZipArchive::new(file).map_err(|e| AppError::ExcelParse(e.to_string()))?;

//     // ── Step 1: 读必要的 xml 字节（单线程，ZipArchive 不是 Send）──
//     let workbook_bytes = read_zip_entry(&mut zip, "xl/workbook.xml")
//         .map_err(|e| AppError::ExcelParse(e.to_string()))?;
//     let rel_bytes = read_zip_entry(&mut zip, "xl/_rels/workbook.xml.rels")
//         .map_err(|e| AppError::ExcelParse(e.to_string()))?;
//     // sharedStrings 可能不存在（全数字的 xlsx）
//     let shared_bytes = read_optional_zip_entry(&mut zip, "xl/sharedStrings.xml")
//         .map_err(|e| AppError::ExcelParse(e.to_string()))?;

//     // ── Step 2: 解析 workbook + rels，得到 sheet 列表──────────────────────
//     let sheet_entries = parse_workbook_and_relationships(&workbook_bytes, &rel_bytes)
//         .map_err(|e| AppError::ExcelParse(e.to_string()))?;

//     // ── Step 3: 读取各 sheet xml 字节（单线程）─────────────────────────
//     let mut sheet_bytes: Vec<(String, Vec<u8>)> = Vec::with_capacity(sheet_entries.len());
//     for entry in &sheet_entries {
//         let bytes = read_zip_entry(&mut zip, &entry.path)
//             .map_err(|e| AppError::ExcelParse(e.to_string()))?;
//         sheet_bytes.push((entry.name.clone(), bytes));
//     }

//     // ── Step 4: 解析 sharedStrings（单线程）─────────────────────────────
//     let shared: Arc<Vec<String>> = Arc::new(match shared_bytes {
//         Some(b) => parse_shared_strings(&b).map_err(|e| AppError::ExcelParse(e.to_string()))?,
//         None => vec![],
//     });

//     // ── Step 5: rayon 并行解析各 sheet ──────────────────────────────
//     let results: Vec<Result<(String, SheetData), AppError>> = sheet_bytes
//         .into_par_iter()
//         .map(|(name, bytes)| {
//             let data = parse_sheet_xml(&bytes, &shared)
//                 .map_err(|e| AppError::ExcelParse(e.to_string()))?;
//             Ok((name, data))
//         })
//         .collect();

//     // ── Step 6: 收集结果 ──────────────────────────────────────────
//     let mut map = HashMap::with_capacity(results.len());
//     for r in results {
//         let (name, data) = r?;
//         map.insert(name, data);
//     }
//     Ok(map)
// }

// // ── zip 读取辅助 ────────────────────────────────────────────────

// fn read_zip_entry<R: Read + Seek>(zip: &mut ZipArchive<R>, name: &str) -> Result<Vec<u8>> {
//     let mut entry = zip
//         .by_name(name)
//         .with_context(|| format!("missing zip entry: {name}"))?;
//     let mut bytes = Vec::with_capacity(entry.size() as usize);
//     entry
//         .read_to_end(&mut bytes)
//         .with_context(|| format!("failed to read zip entry: {name}"))?;
//     Ok(bytes)
// }

// fn read_optional_zip_entry<R: Read + Seek>(
//     zip: &mut ZipArchive<R>,
//     name: &str,
// ) -> Result<Option<Vec<u8>>> {
//     match zip.by_name(name) {
//         Ok(mut entry) => {
//             let mut bytes = Vec::with_capacity(entry.size() as usize);
//             entry
//                 .read_to_end(&mut bytes)
//                 .with_context(|| format!("failed to read zip entry: {name}"))?;
//             Ok(Some(bytes))
//         }
//         // 文件不存在是正常情况（如全数字 xlsx 没有 sharedStrings）
//         Err(zip::result::ZipError::FileNotFound) => Ok(None),
//         Err(err) => {
//             Err(anyhow!(err)).with_context(|| format!("failed to access zip entry: {name}"))
//         }
//     }
// }

// // ── workbook + rels 解析 ──────────────────────────────────────────

// fn parse_workbook_and_relationships(
//     workbook_xml: &[u8],
//     rel_xml: &[u8],
// ) -> Result<Vec<SheetEntry>> {
//     // 先解析 rels，得到 rId -> target 映射
//     let rel_map = parse_relationships(rel_xml)?;

//     let mut reader = Reader::from_reader(Cursor::new(workbook_xml));
//     reader.config_mut().trim_text(true);

//     let mut buf = Vec::new();
//     let mut sheets = Vec::new();

//     loop {
//         match reader.read_event_into(&mut buf) {
//             Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) if e.name().as_ref() == b"sheet" => {
//                 let name = get_attr(e, b"name")?.unwrap_or_else(|| "unknown".to_string());
//                 let rel_id = get_attr(e, b"r:id")?
//                     .ok_or_else(|| anyhow!("missing r:id for sheet '{name}'"))?;
//                 let target = rel_map
//                     .get(&rel_id)
//                     .ok_or_else(|| anyhow!("no relationship target for r:id '{rel_id}'"))?;
//                 sheets.push(SheetEntry {
//                     name,
//                     path: normalize_zip_path(target),
//                 });
//             }
//             Ok(Event::Eof) => break,
//             Ok(_) => {}
//             Err(err) => return Err(anyhow!(err)).context("failed to parse workbook.xml"),
//         }
//         buf.clear();
//     }

//     Ok(sheets)
// }

// fn parse_relationships(xml: &[u8]) -> Result<HashMap<String, String>> {
//     let mut reader = Reader::from_reader(Cursor::new(xml));
//     reader.config_mut().trim_text(true);

//     let mut buf = Vec::new();
//     let mut map = HashMap::new();

//     loop {
//         match reader.read_event_into(&mut buf) {
//             // 匹配尾缀，兼容带命名空间的 Relationship 元素
//             Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e))
//                 if e.name().as_ref().ends_with(b"Relationship") =>
//             {
//                 let id = get_attr(e, b"Id")?;
//                 let target = get_attr(e, b"Target")?;
//                 if let (Some(id), Some(target)) = (id, target) {
//                     map.insert(id, target);
//                 }
//             }
//             Ok(Event::Eof) => break,
//             Ok(_) => {}
//             Err(err) => {
//                 return Err(anyhow!(err)).context("failed to parse workbook.xml.rels");
//             }
//         }
//         buf.clear();
//     }

//     Ok(map)
// }

// // ── sharedStrings 解析 ──────────────────────────────────────────

// fn parse_shared_strings(xml: &[u8]) -> Result<Vec<String>> {
//     let mut reader = Reader::from_reader(Cursor::new(xml));
//     // trim_text(false): 保留共享字符串中的空白字符
//     reader.config_mut().trim_text(false);

//     let mut buf = Vec::new();
//     let mut shared_strings = Vec::new();
//     let mut current_text = String::new();
//     let mut inside_si = false;
//     let mut inside_t = false;

//     loop {
//         match reader.read_event_into(&mut buf) {
//             Ok(Event::Start(ref e)) if e.name().as_ref() == b"si" => {
//                 inside_si = true;
//                 current_text.clear();
//             }
//             Ok(Event::End(ref e)) if e.name().as_ref() == b"si" => {
//                 if inside_si {
//                     shared_strings.push(current_text.clone());
//                     current_text.clear();
//                     inside_si = false;
//                 }
//             }
//             Ok(Event::Start(ref e)) if e.name().as_ref() == b"t" => {
//                 inside_t = true;
//             }
//             Ok(Event::End(ref e)) if e.name().as_ref() == b"t" => {
//                 inside_t = false;
//             }
//             Ok(Event::Text(ref text)) if inside_si && inside_t => {
//                 // xml_content() 返回已 unescape 的 Cow<str>
//                 current_text.push_str(
//                     &text
//                         .xml_content()
//                         .context("failed to decode shared string text")?,
//                 );
//             }
//             Ok(Event::CData(ref text)) if inside_si && inside_t => {
//                 // CData 不需要 unescape
//                 current_text.push_str(
//                     &text
//                         .xml_content()
//                         .context("failed to decode shared string cdata")?,
//                 );
//             }
//             Ok(Event::Eof) => break,
//             Ok(_) => {}
//             Err(err) => {
//                 return Err(anyhow!(err)).context("failed to parse sharedStrings.xml");
//             }
//         }
//         buf.clear();
//     }

//     Ok(shared_strings)
// }

// // ── sheet xml 解析 ──────────────────────────────────────────────

// fn parse_sheet_xml(bytes: &[u8], shared_strings: &[String]) -> Result<SheetData> {
//     let mut reader = Reader::from_reader(Cursor::new(bytes));
//     // trim_text(false): 保留单元格内容的原始空白
//     reader.config_mut().trim_text(false);

//     let mut buf = Vec::new();
//     let mut rows: Vec<SheetRow> = Vec::new();
//     let mut cur_row: SheetRow = Vec::new();
//     let mut scratch = String::new();

//     let mut cur_col_idx: usize = 0;
//     let mut cur_cell_type: Option<String> = None;
//     let mut in_value = false;
//     let mut in_row = false;

//     loop {
//         match reader.read_event_into(&mut buf) {
//             // ── <row> ────────────────────────────────────────────────────────────────
//             Ok(Event::Start(ref e)) if e.name().as_ref() == b"row" => {
//                 cur_row = Vec::new();
//                 in_row = true;
//             }

//             // ── <c> 单元格开始 ───────────────────────────────────────────────
//             Ok(Event::Start(ref e)) if e.name().as_ref() == b"c" => {
//                 scratch.clear();
//                 // cur_cell_type = None;

//                 // 列地址 r="C5" -> col_idx=2
//                 if let Some(cell_ref) = get_attr(e, b"r")? {
//                     cur_col_idx = col_ref_to_idx(&cell_ref);
//                 }
//                 // 单元格类型 t="s"/"str"/"inlineStr"/"b"
//                 cur_cell_type = get_attr(e, b"t")?;

//                 // 补齐稀疏列（xlsx 只存非空单元格）
//                 while cur_row.len() < cur_col_idx {
//                     cur_row.push(CellValue::Empty);
//                 }
//             }

//             // ── <v> 或 <t>（inlineStr）开始 ──────────────────────────────────
//             Ok(Event::Start(ref e)) if matches!(e.name().as_ref(), b"v" | b"t") => {
//                 in_value = true;
//                 scratch.clear();
//             }

//             // ── 文本内容 ──────────────────────────────────────────────────────
//             Ok(Event::Text(ref text)) if in_value => {
//                 scratch.push_str(
//                     &text
//                         .xml_content()
//                         .context("failed to decode sheet cell text")?,
//                 );
//             }
//             Ok(Event::CData(ref text)) if in_value => {
//                 scratch.push_str(
//                     &text
//                         .xml_content()
//                         .context("failed to decode sheet cell cdata")?,
//                 );
//             }

//             // ── </v> 或 </t> 结束：解析并写入单元格值 ──────────────────────
//             Ok(Event::End(ref e)) if in_value && matches!(e.name().as_ref(), b"v" | b"t") => {
//                 in_value = false;
//                 let value = resolve_cell_value(&scratch, cur_cell_type.as_deref(), shared_strings);
//                 cur_row.push(value);
//             }

//             // ── </c> 单元格结束 ───────────────────────────────────────────────
//             Ok(Event::End(ref e)) if e.name().as_ref() == b"c" => {
//                 // 空单元格（无 <v>）：已在 <c> 开始时补齐，这里确保占位
//                 if cur_row.len() == cur_col_idx {
//                     cur_row.push(CellValue::Empty);
//                 }
//                 cur_cell_type = None;
//             }

//             // ── </row> 行结束 ─────────────────────────────────────────────────
//             Ok(Event::End(ref e)) if e.name().as_ref() == b"row" => {
//                 if in_row {
//                     rows.push(std::mem::take(&mut cur_row));
//                     in_row = false;
//                 }
//             }

//             Ok(Event::Eof) => break,
//             Err(err) => {
//                 return Err(anyhow!(err)).context("failed to parse worksheet xml");
//             }
//             Ok(_) => {}
//         }
//         buf.clear();
//     }

//     Ok(SheetData { rows })
// }

// /// 根据单元格类型和原始内容解析为 CellValue
// #[inline]
// fn resolve_cell_value(
//     scratch: &str,
//     cell_type: Option<&str>,
//     shared_strings: &[String],
// ) -> CellValue {
//     match cell_type {
//         Some("s") => {
//             // shared string 索引
//             scratch
//                 .trim()
//                 .parse::<usize>()
//                 .ok()
//                 .and_then(|i| shared_strings.get(i))
//                 .map(|s| CellValue::Str(s.clone()))
//                 .unwrap_or(CellValue::Empty)
//         }
//         Some("str") | Some("inlineStr") => {
//             if scratch.is_empty() {
//                 CellValue::Empty
//             } else {
//                 CellValue::Str(scratch.to_owned())
//             }
//         }
//         Some("b") => {
//             // bool 也当字符串存，parser 层不需要 bool
//             CellValue::Str(scratch.to_owned())
//         }
//         _ => {
//             // 默认为数字（包括日期序列号）
//             let s = scratch.trim();
//             if s.is_empty() {
//                 CellValue::Empty
//             } else {
//                 s.parse::<f64>()
//                     .map(CellValue::Num)
//                     .unwrap_or_else(|_| CellValue::Str(s.to_owned()))
//             }
//         }
//     }
// }

// // ── XML 属性提取辅助 ─────────────────────────────────────────

// /// 从 XML 元素中提取指定属性的字符串值。
// ///
// /// 使用 `element.decoder()` 而非整个 reader，这是 quick-xml 0.36+ 的正确用法。
// fn get_attr(element: &BytesStart<'_>, key: &[u8]) -> Result<Option<String>> {
//     for attr in element.attributes() {
//         let attr = attr.context("failed to read xml attribute")?;
//         if attr.key.as_ref() == key {
//             let value = attr
//                 .decode_and_unescape_value(element.decoder())
//                 .context("failed to decode xml attribute value")?;
//             return Ok(Some(value.into_owned()));
//         }
//     }
//     Ok(None)
// }

// // ── 工具函数 ────────────────────────────────────────────────────

// /// 列字母转索引："A"->0, "B"->1, "Z"->25, "AA"->26
// fn col_ref_to_idx(cell_ref: &str) -> usize {
//     cell_ref
//         .bytes()
//         .take_while(|b| b.is_ascii_alphabetic())
//         .fold(0usize, |acc, b| acc * 26 + (b - b'A') as usize + 1)
//         .saturating_sub(1)
// }

// /// 标准化 zip 路径：确保以 "xl/" 开头
// fn normalize_zip_path(target: &str) -> String {
//     let t = target.trim_start_matches('/');
//     if t.starts_with("xl/") {
//         t.to_string()
//     } else {
//         format!("xl/{t}")
//     }
// }
// src-tauri/src/excel_reader.rs
//
// 使用 zip + quick-xml 直接解析 xlsx。
// 核心目标：
//   1. sharedStrings 只解析一次
//   2. 主线程顺序读取 zip entry（ZipArchive 不是 Send）
//   3. 各 sheet XML 字节交给 rayon 并行解析
//   4. 避免高频属性解析中的 String 分配
//   5. shared string 单元格复用 Arc<str>，避免重复 clone 大字符串
//
// 对外保持兼容：
//   - CellValue::Empty / Str / Num
//   - pub type SheetRow = Vec<CellValue>
//   - SheetData.rows / data_rows()

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

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum CellType {
    Number,
    SharedString,
    InlineStr,
    Str,
    Bool,
}

fn parse_sheet_xml(bytes: &[u8]) -> Result<SheetData> {
    let mut reader = Reader::from_reader(Cursor::new(bytes));
    reader.config_mut().trim_text(false);

    let mut buf = Vec::new();

    let mut rows: Vec<SheetRow> = Vec::new();
    let mut cur_row: SheetRow = Vec::with_capacity(64);
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

            Ok(Event::Start(ref e)) if in_row && e.name().as_ref() == b"c" => {
                scratch.clear();

                cur_col_idx = usize::MAX;
                cur_cell_type = CellType::Number;
                cell_had_value = false;
                in_cell = true;
                in_value = false;

                let (col_idx, cell_type) = parse_cell_attrs(e);
                cur_col_idx = col_idx;
                cur_cell_type = cell_type;

                if cur_col_idx != usize::MAX {
                    while cur_row.len() < cur_col_idx {
                        cur_row.push(CellValue::Empty);
                    }
                }
            }

            Ok(Event::Empty(ref e)) if in_row && e.name().as_ref() == b"c" => {
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

            Ok(Event::End(ref e)) if in_value && matches!(e.name().as_ref(), b"v" | b"t") => {
                in_value = false;
                cell_had_value = true;

                let value = resolve_cell_value(&scratch, cur_cell_type);
                cur_row.push(value);
            }

            Ok(Event::End(ref e)) if in_cell && e.name().as_ref() == b"c" => {
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
                    rows.push(std::mem::replace(
                        &mut cur_row,
                        Vec::with_capacity(64),
                    ));

                    in_row = false;
                    in_cell = false;
                    in_value = false;
                    cell_had_value = false;
                    cur_col_idx = usize::MAX;
                    cur_cell_type = CellType::Number;
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
            if scratch.trim().is_empty() {
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
