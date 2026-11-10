//! 在不破坏格式的前提下向 Excel 模版（税务局提供的 .xlsx）写入单元格。
//!
//! rust_xlsxwriter 只能新建文件、calamine 只能读，因此这里直接改 OOXML：
//!   - 用 zip 逐个复制包内文件，只改目标 sheet 的 XML
//!   - 目标单元格保留原有样式 s="…"，替换值（数字 <v>，文本 inlineStr）
//!   - 不存在的行 / 单元格按顺序插入
//!   - 删除 calcChain.xml，并设置 fullCalcOnLoad，打开时 Excel 自动重算模版里的合计公式
//!
//! 限制：不要把共享公式(shared formula)的主单元格设为写入区域；.xls 模版请先另存为 .xlsx。

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

use anyhow::{anyhow, bail, Context, Result};
use quick_xml::events::{BytesStart, Event};
use quick_xml::{Reader, Writer};
use regex::Regex;
use zip::write::FileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

#[derive(Debug, Clone)]
pub enum CellValue {
    Number(f64),
    Text(String),
}

/// row(1-based) → col(1-based) → value
pub type CellMap = BTreeMap<u32, BTreeMap<u32, CellValue>>;

pub fn set_cell(map: &mut CellMap, cell_ref: &str, v: CellValue) -> Result<()> {
    let (r, c) = parse_cell_ref(cell_ref).ok_or_else(|| anyhow!("无效单元格地址：{cell_ref}"))?;
    map.entry(r).or_default().insert(c, v);
    Ok(())
}

pub fn parse_col_letters(s: &str) -> Option<u32> {
    let s = s.trim().to_ascii_uppercase();
    if s.is_empty() || s.len() > 3 || !s.bytes().all(|b| b.is_ascii_uppercase()) {
        return None;
    }
    Some(s.bytes().fold(0u32, |acc, b| acc * 26 + (b - b'A' + 1) as u32))
}

pub fn col_letters(mut col: u32) -> String {
    let mut out = Vec::new();
    while col > 0 {
        let rem = (col - 1) % 26;
        out.push((b'A' + rem as u8) as char);
        col = (col - 1) / 26;
    }
    out.iter().rev().collect()
}

pub fn parse_cell_ref(r: &str) -> Option<(u32, u32)> {
    let r = r.trim().replace('$', "");
    let split = r.find(|c: char| c.is_ascii_digit())?;
    let col = parse_col_letters(&r[..split])?;
    let row: u32 = r[split..].parse().ok()?;
    (row > 0).then_some((row, col))
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn xml_unescape(s: &str) -> String {
    s.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'").replace("&amp;", "&")
}

fn read_entry(zip: &mut ZipArchive<File>, name: &str) -> Result<String> {
    let mut f = zip.by_name(name).with_context(|| format!("模版缺少 {name}，请确认是 .xlsx 文件"))?;
    let mut s = String::new();
    f.read_to_string(&mut s)?;
    Ok(s)
}

fn attr_of(tag: &str, name: &str) -> Option<String> {
    let re = Regex::new(&format!(r#"\s{}="([^"]*)""#, regex::escape(name))).ok()?;
    re.captures(tag).map(|c| xml_unescape(&c[1]))
}

/// 返回目标 sheet 在包内的路径，如 xl/worksheets/sheet1.xml
fn resolve_sheet_path(workbook: &str, rels: &str, sheet_name: Option<&str>) -> Result<String> {
    let sheet_re = Regex::new(r"<(?:\w+:)?sheet\s[^>]*>")?;
    let rid_re = Regex::new(r#"\s\w+:id="([^"]+)""#)?;
    let mut chosen: Option<String> = None;
    let mut names = Vec::new();
    for m in sheet_re.find_iter(workbook) {
        let tag = m.as_str();
        let name = attr_of(tag, "name").unwrap_or_default();
        let rid = rid_re.captures(tag).map(|c| c[1].to_string());
        names.push(name.clone());
        let wanted = match sheet_name {
            None => chosen.is_none(),
            Some(n) => n.trim() == name,
        };
        if wanted && chosen.is_none() {
            chosen = rid;
        }
    }
    let rid = chosen.ok_or_else(|| anyhow!("模版中找不到工作表 {:?}，现有：{}", sheet_name, names.join("、")))?;

    let rel_re = Regex::new(r"<(?:\w+:)?Relationship\s[^>]*>")?;
    for m in rel_re.find_iter(rels) {
        let tag = m.as_str();
        if attr_of(tag, "Id").as_deref() == Some(rid.as_str()) {
            let target = attr_of(tag, "Target").ok_or_else(|| anyhow!("关系缺少 Target"))?;
            return Ok(match target.strip_prefix('/') {
                Some(abs) => abs.to_string(),
                None => format!("xl/{target}"),
            });
        }
    }
    bail!("模版中找不到工作表关系 {rid}")
}

pub fn fill_template(template: &Path, output: &Path, sheet_name: Option<&str>, cells: &CellMap) -> Result<()> {
    let ext = template.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    if ext != "xlsx" && ext != "xlsm" {
        bail!("模版必须是 .xlsx（.xls 请先用 Excel 另存为 .xlsx）");
    }
    let mut zip = ZipArchive::new(File::open(template).with_context(|| format!("打开模版失败：{}", template.display()))?)?;
    let workbook_xml = read_entry(&mut zip, "xl/workbook.xml")?;
    let rels_xml = read_entry(&mut zip, "xl/_rels/workbook.xml.rels")?;
    let sheet_path = resolve_sheet_path(&workbook_xml, &rels_xml, sheet_name)?;

    let mut zw = ZipWriter::new(File::create(output).with_context(|| format!("无法写入：{}", output.display()))?);
    let opts = FileOptions::default().compression_method(CompressionMethod::Deflated);

    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        let name = entry.name().to_string();
        if entry.is_dir() {
            zw.add_directory(name, opts)?;
            continue;
        }
        if name == "xl/calcChain.xml" {
            continue;
        }
        let mut buf = Vec::new();
        entry.read_to_end(&mut buf)?;
        let data = if name == sheet_path {
            patch_sheet(&buf, cells)?
        } else if name == "xl/workbook.xml" {
            patch_workbook(&String::from_utf8(buf)?).into_bytes()
        } else if name == "[Content_Types].xml" || name == "xl/_rels/workbook.xml.rels" {
            strip_calc_chain(&String::from_utf8(buf)?)?.into_bytes()
        } else {
            buf
        };
        zw.start_file(name, opts)?;
        zw.write_all(&data)?;
    }
    zw.finish()?;
    Ok(())
}

fn patch_workbook(xml: &str) -> String {
    if let Some(pos) = xml.find("<calcPr") {
        if xml[pos..].split('>').next().map_or(false, |t| t.contains("fullCalcOnLoad")) {
            return xml.to_string();
        }
        let mut s = xml.to_string();
        s.insert_str(pos + "<calcPr".len(), r#" fullCalcOnLoad="1""#);
        return s;
    }
    for anchor in ["</definedNames>", "</sheets>"] {
        if let Some(pos) = xml.find(anchor) {
            let mut s = xml.to_string();
            s.insert_str(pos + anchor.len(), r#"<calcPr fullCalcOnLoad="1"/>"#);
            return s;
        }
    }
    xml.to_string()
}

fn strip_calc_chain(xml: &str) -> Result<String> {
    let re = Regex::new(r"<(?:\w+:)?(?:Override|Relationship)\s[^>]*calcChain\.xml[^>]*/>")?;
    Ok(re.replace_all(xml, "").into_owned())
}

fn attr(e: &BytesStart, key: &[u8]) -> Option<String> {
    e.attributes()
        .flatten()
        .find(|a| a.key.as_ref() == key)
        .map(|a| String::from_utf8_lossy(&a.value).into_owned())
}

fn cell_xml(row: u32, col: u32, style: Option<&str>, v: &CellValue) -> String {
    let r = format!("{}{}", col_letters(col), row);
    let s = style.map(|s| format!(r#" s="{s}""#)).unwrap_or_default();
    match v {
        CellValue::Number(n) if n.is_finite() => format!(r#"<c r="{r}"{s}><v>{n}</v></c>"#),
        CellValue::Number(_) => format!(r#"<c r="{r}"{s}/>"#),
        CellValue::Text(t) => {
            format!(r#"<c r="{r}"{s} t="inlineStr"><is><t xml:space="preserve">{}</t></is></c>"#, xml_escape(t))
        }
    }
}

fn raw(w: &mut Writer<Vec<u8>>, s: &str) {
    w.get_mut().extend_from_slice(s.as_bytes());
}

fn write_new_row(w: &mut Writer<Vec<u8>>, row: u32, cells: &BTreeMap<u32, CellValue>) {
    raw(w, &format!(r#"<row r="{row}">"#));
    for (c, v) in cells {
        raw(w, &cell_xml(row, *c, None, v));
    }
    raw(w, "</row>");
}

/// 把 pending 中行号 < upto 的行作为新行写出
fn flush_rows_before(w: &mut Writer<Vec<u8>>, pending: &mut CellMap, upto: u32) {
    let keys: Vec<u32> = pending.range(..upto).map(|(k, _)| *k).collect();
    for k in keys {
        if let Some(cells) = pending.remove(&k) {
            write_new_row(w, k, &cells);
        }
    }
}

fn flush_cells_before(w: &mut Writer<Vec<u8>>, row: u32, cells: &mut BTreeMap<u32, CellValue>, upto: u32) {
    let keys: Vec<u32> = cells.range(..upto).map(|(k, _)| *k).collect();
    for k in keys {
        if let Some(v) = cells.remove(&k) {
            raw(w, &cell_xml(row, k, None, &v));
        }
    }
}

fn patch_sheet(xml: &[u8], cells: &CellMap) -> Result<Vec<u8>> {
    let mut reader = Reader::from_reader(xml);
    let mut w = Writer::new(Vec::with_capacity(xml.len() + 8192));
    let mut pending: CellMap = cells.clone();

    let mut in_sheet_data = false;
    // 当前行：(行号, 本行待写单元格, 上一个单元格列号)
    let mut cur: Option<(u32, BTreeMap<u32, CellValue>, u32)> = None;
    let mut last_row: u32 = 0;
    let mut skip_depth: usize = 0;
    let mut buf = Vec::new();

    loop {
        // into_owned：解除对 buf 的借用，便于每轮 clear
        let ev = reader.read_event_into(&mut buf)?.into_owned();

        // 正在跳过被替换的原单元格内容
        if skip_depth > 0 {
            match ev {
                Event::Start(_) => skip_depth += 1,
                Event::End(_) => skip_depth -= 1,
                Event::Eof => bail!("模版 sheet XML 不完整"),
                _ => {}
            }
            buf.clear();
            continue;
        }

        match ev {
            Event::Eof => break,

            Event::Start(e) if e.local_name().as_ref() == b"sheetData" => {
                in_sheet_data = true;
                w.write_event(Event::Start(e))?;
            }
            Event::Empty(e) if e.local_name().as_ref() == b"sheetData" => {
                let name = String::from_utf8_lossy(e.name().as_ref()).into_owned();
                raw(&mut w, &format!("<{name}>"));
                flush_rows_before(&mut w, &mut pending, u32::MAX);
                raw(&mut w, &format!("</{name}>"));
            }
            Event::End(e) if e.local_name().as_ref() == b"sheetData" => {
                flush_rows_before(&mut w, &mut pending, u32::MAX);
                in_sheet_data = false;
                w.write_event(Event::End(e))?;
            }

            Event::Start(e) if in_sheet_data && cur.is_none() && e.local_name().as_ref() == b"row" => {
                let r = attr(&e, b"r").and_then(|s| s.parse().ok()).unwrap_or(last_row + 1);
                last_row = r;
                flush_rows_before(&mut w, &mut pending, r);
                cur = Some((r, pending.remove(&r).unwrap_or_default(), 0));
                w.write_event(Event::Start(e))?;
            }
            Event::Empty(e) if in_sheet_data && cur.is_none() && e.local_name().as_ref() == b"row" => {
                let r = attr(&e, b"r").and_then(|s| s.parse().ok()).unwrap_or(last_row + 1);
                last_row = r;
                flush_rows_before(&mut w, &mut pending, r);
                match pending.remove(&r) {
                    Some(cells) => {
                        w.write_event(Event::Start(e.clone()))?;
                        for (c, v) in &cells {
                            raw(&mut w, &cell_xml(r, *c, None, v));
                        }
                        let name = String::from_utf8_lossy(e.name().as_ref()).into_owned();
                        raw(&mut w, &format!("</{name}>"));
                    }
                    None => w.write_event(Event::Empty(e))?,
                }
            }
            Event::End(e) if cur.is_some() && e.local_name().as_ref() == b"row" => {
                if let Some((r, mut rest, _)) = cur.take() {
                    flush_cells_before(&mut w, r, &mut rest, u32::MAX);
                }
                w.write_event(Event::End(e))?;
            }

            Event::Start(e) if cur.is_some() && e.local_name().as_ref() == b"c" => {
                let (r, cells, last_col) = cur.as_mut().unwrap();
                let col = attr(&e, b"r").and_then(|s| parse_cell_ref(&s)).map(|x| x.1).unwrap_or(*last_col + 1);
                *last_col = col;
                let r = *r;
                flush_cells_before(&mut w, r, cells, col);
                match cells.remove(&col) {
                    Some(v) => {
                        raw(&mut w, &cell_xml(r, col, attr(&e, b"s").as_deref(), &v));
                        skip_depth = 1;
                    }
                    None => w.write_event(Event::Start(e))?,
                }
            }
            Event::Empty(e) if cur.is_some() && e.local_name().as_ref() == b"c" => {
                let (r, cells, last_col) = cur.as_mut().unwrap();
                let col = attr(&e, b"r").and_then(|s| parse_cell_ref(&s)).map(|x| x.1).unwrap_or(*last_col + 1);
                *last_col = col;
                let r = *r;
                flush_cells_before(&mut w, r, cells, col);
                match cells.remove(&col) {
                    Some(v) => raw(&mut w, &cell_xml(r, col, attr(&e, b"s").as_deref(), &v)),
                    None => w.write_event(Event::Empty(e))?,
                }
            }

            other => w.write_event(other)?,
        }
        buf.clear();
    }
    Ok(w.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refs() {
        assert_eq!(parse_cell_ref("B5"), Some((5, 2)));
        assert_eq!(parse_cell_ref("$AA$10"), Some((10, 27)));
        assert_eq!(col_letters(28), "AB");
    }

    #[test]
    fn patch_keeps_style_and_inserts() {
        let xml = br#"<worksheet><sheetData><row r="2"><c r="A2" s="3"><v>1</v></c><c r="C2" s="4"/></row><row r="5"/></sheetData></worksheet>"#;
        let mut m = CellMap::new();
        set_cell(&mut m, "A2", CellValue::Text("x&y".into())).unwrap();
        set_cell(&mut m, "B2", CellValue::Number(2.5)).unwrap();
        set_cell(&mut m, "A3", CellValue::Number(3.0)).unwrap();
        set_cell(&mut m, "B5", CellValue::Number(5.0)).unwrap();
        set_cell(&mut m, "A9", CellValue::Number(9.0)).unwrap();
        let out = String::from_utf8(patch_sheet(xml, &m).unwrap()).unwrap();
        assert!(out.contains(r#"<c r="A2" s="3" t="inlineStr"><is><t xml:space="preserve">x&amp;y</t></is></c><c r="B2"><v>2.5</v></c><c r="C2" s="4"/>"#));
        assert!(out.contains(r#"</row><row r="3"><c r="A3"><v>3</v></c></row><row r="5"><c r="B5"><v>5</v></c></row><row r="9">"#));
    }
}
