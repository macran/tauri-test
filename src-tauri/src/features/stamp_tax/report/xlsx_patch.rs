//! 最小改动的 xlsx 模板修改器（zip + quick-xml）。
//!
//! 原则：只重写指定的 sheet XML，其余 zip 条目原样 raw copy（不解压不重压），
//! 以最大程度保留税务局模板的命名区域、数据验证、customXml 等内容。

use std::collections::HashMap;
use std::fmt::Write as _;
use std::fs::File;
use std::io::{BufWriter, Cursor, Read, Write};
use std::path::Path;

use quick_xml::events::{BytesStart, Event};
use quick_xml::{Reader, Writer};
use zip::write::FileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use super::ReportError;

/// 单元格值
#[derive(Debug, Clone)]
pub enum CellValue {
    /// 数值：按字符串原样写入 `<v>`，全程不经过 f64
    Number(String),
    /// 文本：以 inlineStr 写入，不需要改动 sharedStrings.xml
    Text(String),
}

/// 对一个 sheet XML 的修改指令
#[derive(Debug, Default)]
pub struct SheetPatch {
    /// 删除行号 >= 该值的所有 `<row>`
    pub drop_rows_from: Option<u32>,
    /// 替换指定单元格的值（保留原单元格的样式 s），key 例如 "D4"
    pub replace_cells: HashMap<String, CellValue>,
    /// 在 `</sheetData>` 之前追加的行 XML
    pub append_rows: String,
    /// 把 `<dimension ref>` 的结束行改为该值
    pub dimension_last_row: Option<u32>,
    /// 数据验证中结束行等于 .0 的区域，扩展到 .1
    pub extend_validation: Option<(u32, u32)>,
}

/// 生成一个 `<c>` 单元格的 XML
pub fn cell_xml(out: &mut String, cell_ref: &str, style: u32, value: Option<&CellValue>) {
    match value {
        None => {
            let _ = write!(out, r#"<c r="{cell_ref}" s="{style}"/>"#);
        }
        Some(CellValue::Number(v)) => {
            let _ = write!(out, r#"<c r="{cell_ref}" s="{style}"><v>{v}</v></c>"#);
        }
        Some(CellValue::Text(t)) => {
            let _ = write!(
                out,
                r#"<c r="{cell_ref}" s="{style}" t="inlineStr"><is><t xml:space="preserve">{}</t></is></c>"#,
                quick_xml::escape::escape(t.as_str())
            );
        }
    }
}

/// 已打开的模板
pub struct XlsxTemplate<'a> {
    archive: ZipArchive<Cursor<&'a [u8]>>,
    /// sheet 名称 -> zip 内路径，例如 "采集表" -> "xl/worksheets/sheet1.xml"
    sheets: HashMap<String, String>,
}

impl<'a> XlsxTemplate<'a> {
    pub fn open(bytes: &'a [u8]) -> Result<Self, ReportError> {
        let mut archive = ZipArchive::new(Cursor::new(bytes))?;
        let workbook = read_entry(&mut archive, "xl/workbook.xml")?;
        let rels = read_entry(&mut archive, "xl/_rels/workbook.xml.rels")?;
        let sheets = resolve_sheet_paths(&workbook, &rels)?;
        Ok(Self { archive, sheets })
    }

    pub fn sheet_path(&self, sheet_name: &str) -> Result<String, ReportError> {
        self.sheets
            .get(sheet_name)
            .cloned()
            .ok_or_else(|| ReportError::SheetNotFound(sheet_name.to_owned()))
    }

    pub fn read_text(&mut self, path: &str) -> Result<String, ReportError> {
        read_entry(&mut self.archive, path)
    }

    /// 写出新文件：`patched` 中的条目用新内容，其余条目原样拷贝。
    /// 先写临时文件再 rename，避免写到一半留下损坏文件。
    pub fn save_as(
        mut self,
        patched: &HashMap<String, Vec<u8>>,
        output: &Path,
    ) -> Result<(), ReportError> {
        let tmp = output.with_extension("xlsx.tmp");
        let result = (|| -> Result<(), ReportError> {
            let mut zw = ZipWriter::new(BufWriter::new(File::create(&tmp)?));
            let opts = FileOptions::default().compression_method(CompressionMethod::Deflated);
            for i in 0..self.archive.len() {
                let entry = self.archive.by_index_raw(i)?;
                let name = entry.name().to_owned();
                match patched.get(&name) {
                    Some(bytes) => {
                        drop(entry);
                        zw.start_file(name, opts)?;
                        zw.write_all(bytes)?;
                    }
                    None => zw.raw_copy_file(entry)?,
                }
            }
            zw.finish()?.flush()?;
            Ok(())
        })();

        if let Err(e) = result {
            let _ = std::fs::remove_file(&tmp);
            return Err(e);
        }
        std::fs::rename(&tmp, output).map_err(|source| {
            let _ = std::fs::remove_file(&tmp);
            ReportError::OutputLocked { path: output.display().to_string(), source }
        })
    }
}

/// 按 SheetPatch 流式改写 sheet XML，未涉及的节点逐个事件原样透传。
pub fn patch_sheet_xml(xml: &str, patch: &SheetPatch) -> Result<Vec<u8>, ReportError> {
    let mut reader = Reader::from_str(xml);
    let mut writer = Writer::new(Vec::with_capacity(xml.len() + patch.append_rows.len()));

    loop {
        let event = reader.read_event()?;
        match &event {
            Event::Eof => break,

            Event::Start(e) | Event::Empty(e) => {
                let is_empty = matches!(event, Event::Empty(_));
                match e.local_name().as_ref() {
                    b"row" => {
                        let row_no = attr(e, b"r").and_then(|v| v.parse::<u32>().ok());
                        if let (Some(from), Some(r)) = (patch.drop_rows_from, row_no) {
                            if r >= from {
                                if !is_empty {
                                    reader.read_to_end(e.name())?; // 跳过整行
                                }
                                continue;
                            }
                        }
                    }
                    b"c" => {
                        if let Some(cell_ref) = attr(e, b"r") {
                            if let Some(value) = patch.replace_cells.get(&cell_ref) {
                                if !is_empty {
                                    reader.read_to_end(e.name())?; // 丢弃旧值
                                }
                                let style = attr(e, b"s").and_then(|s| s.parse().ok()).unwrap_or(0);
                                let mut s = String::new();
                                cell_xml(&mut s, &cell_ref, style, Some(value));
                                writer.get_mut().write_all(s.as_bytes())?;
                                continue;
                            }
                        }
                    }
                    b"dimension" => {
                        if let (Some(last), Some(r)) = (patch.dimension_last_row, attr(e, b"ref")) {
                            write_with_attr(&mut writer, e, is_empty, b"ref", &set_end_row(&r, last))?;
                            continue;
                        }
                    }
                    b"dataValidation" => {
                        if let (Some((from, to)), Some(sqref)) = (patch.extend_validation, attr(e, b"sqref")) {
                            write_with_attr(&mut writer, e, is_empty, b"sqref", &extend_sqref(&sqref, from, to))?;
                            continue;
                        }
                    }
                    b"sheetData" if is_empty && !patch.append_rows.is_empty() => {
                        // <sheetData/> 的情况：展开为 <sheetData>...</sheetData>
                        writer.get_mut().write_all(b"<sheetData>")?;
                        writer.get_mut().write_all(patch.append_rows.as_bytes())?;
                        writer.get_mut().write_all(b"</sheetData>")?;
                        continue;
                    }
                    _ => {}
                }
            }

            Event::End(e) if e.local_name().as_ref() == b"sheetData" => {
                writer.get_mut().write_all(patch.append_rows.as_bytes())?;
            }

            _ => {}
        }
        writer.write_event(event)?;
    }
    Ok(writer.into_inner())
}

/// "D4:E4 M10:M30 I10:K30" + (30 -> 33) => "D4:E4 M10:M33 I10:K33"
pub fn extend_sqref(sqref: &str, from: u32, to: u32) -> String {
    sqref
        .split_whitespace()
        .map(|range| match range.split_once(':') {
            Some((start, end)) => {
                let (col, row) = split_cell_ref(end);
                match row.parse::<u32>() {
                    Ok(r) if r == from => format!("{start}:{col}{to}"),
                    _ => range.to_owned(),
                }
            }
            None => range.to_owned(),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// "A1:AA30" + 33 => "A1:AA33"
fn set_end_row(dimension: &str, last_row: u32) -> String {
    match dimension.split_once(':') {
        Some((start, end)) => format!("{start}:{}{last_row}", split_cell_ref(end).0),
        None => dimension.to_owned(),
    }
}

fn split_cell_ref(cell: &str) -> (&str, &str) {
    let idx = cell.find(|c: char| c.is_ascii_digit()).unwrap_or(cell.len());
    cell.split_at(idx)
}

fn attr(e: &BytesStart, key: &[u8]) -> Option<String> {
    e.attributes()
        .flatten()
        .find(|a| a.key.as_ref() == key)
        .map(|a| String::from_utf8_lossy(&a.value).into_owned())
}

/// 原样复制元素及其属性，仅替换其中一个属性的值
fn write_with_attr(
    writer: &mut Writer<Vec<u8>>,
    e: &BytesStart,
    is_empty: bool,
    key: &[u8],
    value: &str,
) -> Result<(), ReportError> {
    let mut el = BytesStart::new(String::from_utf8_lossy(e.name().as_ref()).into_owned());
    for a in e.attributes().flatten() {
        if a.key.as_ref() == key {
            el.push_attribute((key, value.as_bytes()));
        } else {
            el.push_attribute((a.key.as_ref(), a.value.as_ref()));
        }
    }
    writer.write_event(if is_empty { Event::Empty(el) } else { Event::Start(el) })?;
    Ok(())
}

fn read_entry(archive: &mut ZipArchive<Cursor<&[u8]>>, path: &str) -> Result<String, ReportError> {
    let mut s = String::new();
    archive.by_name(path)?.read_to_string(&mut s)?;
    Ok(s)
}

/// 解析 workbook.xml + workbook.xml.rels，得到 sheet 名称 -> zip 内路径
fn resolve_sheet_paths(workbook: &str, rels: &str) -> Result<HashMap<String, String>, ReportError> {
    let mut targets = HashMap::new(); // rId -> Target
    let mut reader = Reader::from_str(rels);
    loop {
        match reader.read_event()? {
            Event::Eof => break,
            Event::Start(e) | Event::Empty(e) if e.local_name().as_ref() == b"Relationship" => {
                if let (Some(id), Some(target)) = (attr(&e, b"Id"), attr(&e, b"Target")) {
                    let path = match target.strip_prefix('/') {
                        Some(abs) => abs.to_owned(),
                        None => format!("xl/{target}"),
                    };
                    targets.insert(id, path);
                }
            }
            _ => {}
        }
    }

    let mut sheets = HashMap::new();
    let mut reader = Reader::from_str(workbook);
    loop {
        match reader.read_event()? {
            Event::Eof => break,
            Event::Start(e) | Event::Empty(e) if e.local_name().as_ref() == b"sheet" => {
                if let (Some(name), Some(rid)) = (attr(&e, b"name"), attr(&e, b"r:id")) {
                    if let Some(path) = targets.get(&rid) {
                        let name = match quick_xml::escape::unescape(&name) {
                            Ok(c) => c.into_owned(),
                            Err(_) => name.clone(),
                        };
                        sheets.insert(name, path.clone());
                    }
                }
            }
            _ => {}
        }
    }
    Ok(sheets)
}
