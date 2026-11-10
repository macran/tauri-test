//! 普通导出（rust_xlsxwriter）：清单表 + 说明/汇总表

use anyhow::Result;
use rust_xlsxwriter::{Color, Format, FormatAlign, FormatBorder, Workbook, Worksheet};

pub enum Val {
    Text(String),
    Opt(Option<String>),
    /// 金额（分），输出为元，两位小数
    Money(i64),
    Int(i64),
    Num(f64),
    Empty,
}

impl From<&str> for Val {
    fn from(s: &str) -> Self {
        Val::Text(s.to_string())
    }
}
impl From<String> for Val {
    fn from(s: String) -> Self {
        Val::Text(s)
    }
}
impl From<Option<String>> for Val {
    fn from(s: Option<String>) -> Self {
        Val::Opt(s)
    }
}

pub struct Col {
    pub header: &'static str,
    pub width: f64,
}

pub const fn col(header: &'static str, width: f64) -> Col {
    Col { header, width }
}

struct Styles {
    header: Format,
    money: Format,
    int: Format,
    num: Format,
    text: Format,
}

fn styles() -> Styles {
    let base = Format::new().set_border(FormatBorder::Thin).set_border_color(Color::RGB(0xBFBFBF));
    Styles {
        header: base
            .clone()
            .set_bold()
            .set_background_color(Color::RGB(0xDDEBF7))
            .set_align(FormatAlign::Center)
            .set_text_wrap(),
        money: base.clone().set_num_format("#,##0.00"),
        int: base.clone().set_num_format("0"),
        num: base.clone().set_num_format("0.######"),
        text: base,
    }
}

/// 写一张带表头、冻结首行、自动筛选的明细表
pub fn write_table(wb: &mut Workbook, name: &str, cols: &[Col], rows: &[Vec<Val>]) -> Result<()> {
    let st = styles();
    let ws = wb.add_worksheet();
    ws.set_name(name)?;
    for (c, col) in cols.iter().enumerate() {
        ws.write_string_with_format(0, c as u16, col.header, &st.header)?;
        ws.set_column_width(c as u16, col.width)?;
    }
    ws.set_row_height(0, 30)?;
    for (r, row) in rows.iter().enumerate() {
        let r = (r + 1) as u32;
        for (c, v) in row.iter().enumerate() {
            write_val(ws, r, c as u16, v, &st)?;
        }
    }
    ws.set_freeze_panes(1, 0)?;
    if !cols.is_empty() {
        ws.autofilter(0, 0, rows.len() as u32, (cols.len() - 1) as u16)?;
    }
    Ok(())
}

fn write_val(ws: &mut Worksheet, r: u32, c: u16, v: &Val, st: &Styles) -> Result<()> {
    match v {
        Val::Text(s) => {
            ws.write_string_with_format(r, c, s, &st.text)?;
        }
        Val::Opt(Some(s)) => {
            ws.write_string_with_format(r, c, s, &st.text)?;
        }
        Val::Opt(None) | Val::Empty => {
            ws.write_blank(r, c, &st.text)?;
        }
        Val::Money(f) => {
            ws.write_number_with_format(r, c, *f as f64 / 100.0, &st.money)?;
        }
        Val::Int(i) => {
            ws.write_number_with_format(r, c, *i as f64, &st.int)?;
        }
        Val::Num(n) => {
            ws.write_number_with_format(r, c, *n, &st.num)?;
        }
    }
    Ok(())
}

/// 说明页：上半部分键值对，下半部分可选汇总表
pub fn write_info_sheet(
    wb: &mut Workbook,
    name: &str,
    info: &[(&str, String)],
    summary_cols: &[Col],
    summary_rows: &[Vec<Val>],
) -> Result<()> {
    let st = styles();
    let ws = wb.add_worksheet();
    ws.set_name(name)?;
    ws.set_column_width(0, 22)?;
    ws.set_column_width(1, 40)?;
    for (i, (k, v)) in info.iter().enumerate() {
        ws.write_string_with_format(i as u32, 0, *k, &st.header)?;
        ws.write_string_with_format(i as u32, 1, v, &st.text)?;
    }
    if summary_cols.is_empty() {
        return Ok(());
    }
    let start = info.len() as u32 + 2;
    for (c, col) in summary_cols.iter().enumerate() {
        ws.write_string_with_format(start, c as u16, col.header, &st.header)?;
        if c >= 2 {
            ws.set_column_width(c as u16, col.width)?;
        }
    }
    for (r, row) in summary_rows.iter().enumerate() {
        for (c, v) in row.iter().enumerate() {
            write_val(ws, start + 1 + r as u32, c as u16, v, &st)?;
        }
    }
    Ok(())
}
