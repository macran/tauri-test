//! 印花税税源信息采集表生成：基于税务局下载的导入模板，填入汇总数据后另存。

mod xlsx_patch;

use std::collections::HashMap;
use std::path::Path;

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use xlsx_patch::{cell_xml, patch_sheet_xml, CellValue, SheetPatch, XlsxTemplate};

/// 内嵌的税务局模板（放在 src-tauri/templates/ 下）。模板换版时替换该文件并重新编译。
pub const TEMPLATE: &[u8] = include_bytes!("../../templates/stamp_tax_template.xlsx");

const MAIN_SHEET: &str = "采集表";
const SETTLEMENT_SHEET: &str = "实际结算信息";
const COUNTERPARTY_SHEET: &str = "对方书立人信息";

/// 采集表数据起始行（第 9 行是隐藏的字段绑定行）
const FIRST_DATA_ROW: u32 = 10;
/// 模板自带格式/数据验证的最后一行
const TEMPLATE_LAST_ROW: u32 = 30;
/// 另外两张附表：保留 1-3 行（类名、表头、隐藏绑定行），删除示例数据
const SUB_SHEET_FIRST_DATA_ROW: u32 = 4;

const PROPERTY_TRANSFER_CODE: &str = "101110200";

/// 每列样式编号，取自模板第 10 行（避免模板空白行 G/H 列误设为日期格式的问题）
const ROW_STYLES: [(&str, u32); 22] = [
    ("A", 38), ("B", 92), ("C", 92), ("D", 92), ("E", 92), ("F", 93),
    ("G", 94), ("H", 95), ("I", 96), ("J", 96), ("K", 96), ("L", 97),
    ("M", 98), ("N", 99), ("O", 100), ("P", 97), ("Q", 92), ("R", 101),
    ("S", 102), ("T", 103), ("U", 104), ("V", 38),
];

// ───────────────────────── 枚举 ─────────────────────────

/// 申报类型（采集表 K4）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeclareType {
    #[serde(rename = "11")]
    Normal,
    #[serde(rename = "21")]
    SelfCorrection,
}

impl DeclareType {
    pub fn excel_value(self) -> &'static str {
        match self {
            Self::Normal => "11|正常申报",
            Self::SelfCorrection => "21|纳税人自行补正",
        }
    }
}

/// 申报期限类型（采集表 E 列）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PeriodType {
    #[serde(rename = "00")]
    Periodic,
    #[serde(rename = "01")]
    PerTransaction,
}

impl PeriodType {
    pub fn excel_value(self) -> &'static str {
        match self {
            Self::Periodic => "00|按期申报",
            Self::PerTransaction => "01|按次申报",
        }
    }
}

// ───────────────────────── 入参 / 出参 ─────────────────────────

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StampTaxReport {
    /// 所属期起止，前端传 "YYYY-MM-DD"
    pub period_start: NaiveDate,
    pub period_end: NaiveDate,
    pub declare_type: DeclareType,
    /// 已按 (申报期限类型, 税目, 子目) 汇总好的行
    pub lines: Vec<ReportLine>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportLine {
    pub period_type: PeriodType,
    pub item_code: String,
    pub item_name: String,
    pub sub_item_code: Option<String>,
    pub sub_item_name: Option<String>,
    /// 适用税率，百万分之一（0.0003 => 300）
    pub rate_ppm: i64,
    /// 计税金额（不含增值税），单位：分
    pub taxable_fen: i64,
    /// 应税凭证名称；不传则取子目名称，否则取税目名称
    #[serde(default)]
    pub voucher_name: Option<String>,
}

impl ReportLine {
    fn voucher_name(&self) -> String {
        self.voucher_name
            .clone()
            .filter(|s| !s.trim().is_empty())
            .or_else(|| self.sub_item_name.clone())
            .unwrap_or_else(|| self.item_name.clone())
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportSummary {
    pub row_count: usize,
    pub total_taxable_fen: i64,
    pub total_tax_fen: i64,
}

// ───────────────────────── 错误 ─────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum ReportError {
    #[error("模板文件无效：{0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("XML 处理失败：{0}")]
    Xml(#[from] quick_xml::Error),
    #[error("文件读写失败：{0}")]
    Io(#[from] std::io::Error),
    #[error("模板中找不到工作表「{0}」，模板可能已换版")]
    SheetNotFound(String),
    #[error("无法写入 {path}，请确认文件没有在 Excel 中打开（{source}）")]
    OutputLocked { path: String, source: std::io::Error },
    #[error("数据校验失败：{0}")]
    Invalid(String),
    #[error("内部错误：{0}")]
    Internal(String),
}

// Tauri command 的错误需要可序列化，这里直接序列化为中文提示
impl Serialize for ReportError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

// ───────────────────────── Tauri command ─────────────────────────

#[tauri::command]
pub async fn generate_stamp_tax_report(
    output_path: String,
    report: StampTaxReport,
) -> Result<ReportSummary, ReportError> {
    tauri::async_runtime::spawn_blocking(move || generate(TEMPLATE, &report, Path::new(&output_path)))
        .await
        .map_err(|e| ReportError::Internal(e.to_string()))?
}

// ───────────────────────── 核心逻辑 ─────────────────────────

pub fn generate(
    template: &[u8],
    report: &StampTaxReport,
    output: &Path,
) -> Result<ReportSummary, ReportError> {
    validate(report)?;

    let mut tpl = XlsxTemplate::open(template)?;
    let mut patched: HashMap<String, Vec<u8>> = HashMap::new();

    // 1. 采集表
    let (rows_xml, last_row, summary) = build_rows(report);
    let main_path = tpl.sheet_path(MAIN_SHEET)?;
    let main_xml = tpl.read_text(&main_path)?;
    let main_patch = SheetPatch {
        drop_rows_from: Some(FIRST_DATA_ROW),
        replace_cells: HashMap::from([
            ("D4".to_owned(), CellValue::Number(excel_serial(report.period_start).to_string())),
            ("G4".to_owned(), CellValue::Number(excel_serial(report.period_end).to_string())),
            ("K4".to_owned(), CellValue::Text(report.declare_type.excel_value().to_owned())),
        ]),
        append_rows: rows_xml,
        dimension_last_row: Some(last_row),
        extend_validation: Some((TEMPLATE_LAST_ROW, last_row)),
    };
    patched.insert(main_path, patch_sheet_xml(&main_xml, &main_patch)?);

    // 2. 两张附表：清除示例数据
    for name in [SETTLEMENT_SHEET, COUNTERPARTY_SHEET] {
        let path = tpl.sheet_path(name)?;
        let xml = tpl.read_text(&path)?;
        let p = SheetPatch {
            drop_rows_from: Some(SUB_SHEET_FIRST_DATA_ROW),
            dimension_last_row: Some(SUB_SHEET_FIRST_DATA_ROW - 1),
            ..Default::default()
        };
        patched.insert(path, patch_sheet_xml(&xml, &p)?);
    }

    tpl.save_as(&patched, output)?;
    Ok(summary)
}

fn validate(r: &StampTaxReport) -> Result<(), ReportError> {
    let invalid = |m: String| -> Result<(), ReportError> { Err(ReportError::Invalid(m)) };
    let law_effective = NaiveDate::from_ymd_opt(2022, 7, 1).expect("valid date");

    if r.period_start < law_effective {
        return invalid("所属期起不能早于 2022-07-01（《印花税法》施行日）".into());
    }
    if r.period_end < r.period_start {
        return invalid("所属期止不能早于所属期起".into());
    }
    if r.lines.is_empty() {
        return invalid("本期没有需要申报的印花税数据".into());
    }
    for (i, l) in r.lines.iter().enumerate() {
        let n = i + 1;
        if l.taxable_fen < 0 {
            return invalid(format!("第 {n} 行「{}」计税金额为负数", l.item_name));
        }
        if l.rate_ppm <= 0 {
            return invalid(format!("第 {n} 行「{}」税率无效", l.item_name));
        }
        if l.sub_item_code.is_some() != l.sub_item_name.is_some() {
            return invalid(format!("第 {n} 行子目编码与子目名称必须同时提供"));
        }
        match &l.sub_item_code {
            None if l.item_code == PROPERTY_TRANSFER_CODE => {
                return invalid(format!("第 {n} 行：产权转移书据必须指定子目"));
            }
            Some(sub) if sub.split_once("YH").map(|(p, _)| p) != Some(l.item_code.as_str()) => {
                return invalid(format!("第 {n} 行：子目 {sub} 不属于税目 {}", l.item_code));
            }
            _ => {}
        }
    }
    Ok(())
}

/// 返回 (行 XML, 最后一行行号, 汇总信息)
fn build_rows(report: &StampTaxReport) -> (String, u32, ReportSummary) {
    let start = excel_serial(report.period_start).to_string();
    let end = excel_serial(report.period_end).to_string();
    let mut xml = String::new();
    let (mut total_taxable, mut total_tax) = (0i64, 0i64);

    for (i, line) in report.lines.iter().enumerate() {
        let row = FIRST_DATA_ROW + i as u32;
        let tax = compute_tax_fen(line.taxable_fen, line.rate_ppm);
        total_taxable += line.taxable_fen;
        total_tax += tax;

        let mut v: HashMap<&str, CellValue> = HashMap::from([
            ("B", CellValue::Text((i + 1).to_string())),                       // 序号
            ("D", CellValue::Text(line.voucher_name())),                        // 应税凭证名称
            ("E", CellValue::Text(line.period_type.excel_value().to_owned())),  // 申报期限类型
            ("F", CellValue::Number("1".into())),                               // 应税凭证数量
            ("G", CellValue::Text(format!("{}|{}", line.item_code, line.item_name))),
            ("I", CellValue::Number(start.clone())),                            // 税款所属期起
            ("J", CellValue::Number(end.clone())),                              // 税款所属期止
            ("K", CellValue::Number(end.clone())),                              // 书立日期 = 所属期止
            ("L", CellValue::Number(fen_to_str(line.taxable_fen))),             // 计税金额
            ("O", CellValue::Number(ppm_to_str(line.rate_ppm))),                // 税率
            ("P", CellValue::Number(fen_to_str(tax))),                          // 应纳税额
        ]);
        if let (Some(code), Some(name)) = (&line.sub_item_code, &line.sub_item_name) {
            v.insert("H", CellValue::Text(format!("{code}|{name}")));
        }
        push_row(&mut xml, row, &v);
    }

    // 不足模板原有 21 行时补空白行，保持外观一致
    let data_last = FIRST_DATA_ROW + report.lines.len() as u32 - 1;
    let last_row = data_last.max(TEMPLATE_LAST_ROW);
    for row in (data_last + 1)..=last_row {
        push_row(&mut xml, row, &HashMap::new());
    }

    let summary = ReportSummary {
        row_count: report.lines.len(),
        total_taxable_fen: total_taxable,
        total_tax_fen: total_tax,
    };
    (xml, last_row, summary)
}

fn push_row(out: &mut String, row: u32, values: &HashMap<&str, CellValue>) {
    out.push_str(&format!(
        r#"<row r="{row}" s="29" customFormat="1" ht="18" customHeight="1" spans="1:27">"#
    ));
    for (col, style) in ROW_STYLES {
        cell_xml(out, &format!("{col}{row}"), style, values.get(col));
    }
    out.push_str("</row>");
}

// ───────────────────────── 数值工具 ─────────────────────────

/// Excel 1900 日期系统序列号：2024-01-01 => 45292
pub fn excel_serial(d: NaiveDate) -> i64 {
    let epoch = NaiveDate::from_ymd_opt(1899, 12, 30).expect("valid date");
    (d - epoch).num_days()
}

/// 2000050 分 => "20000.50"
pub fn fen_to_str(fen: i64) -> String {
    let sign = if fen < 0 { "-" } else { "" };
    let a = fen.unsigned_abs();
    format!("{sign}{}.{:02}", a / 100, a % 100)
}

/// 300 ppm => "0.000300"
pub fn ppm_to_str(ppm: i64) -> String {
    format!("{}.{:06}", ppm / 1_000_000, ppm % 1_000_000)
}

/// 应纳税额（分），四舍五入
pub fn compute_tax_fen(taxable_fen: i64, rate_ppm: i64) -> i64 {
    ((taxable_fen as i128 * rate_ppm as i128 + 500_000) / 1_000_000) as i64
}

// ───────────────────────── 测试 ─────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use calamine::{open_workbook, Data, Reader, Xlsx};

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    fn line(code: &str, name: &str, sub: Option<(&str, &str)>, ppm: i64, fen: i64) -> ReportLine {
        ReportLine {
            period_type: PeriodType::Periodic,
            item_code: code.into(),
            item_name: name.into(),
            sub_item_code: sub.map(|s| s.0.into()),
            sub_item_name: sub.map(|s| s.1.into()),
            rate_ppm: ppm,
            taxable_fen: fen,
            voucher_name: None,
        }
    }

    #[test]
    fn numbers() {
        assert_eq!(excel_serial(d("2024-01-01")), 45292);
        assert_eq!(excel_serial(d("2022-07-01")), 44743);
        assert_eq!(fen_to_str(2_000_050), "20000.50");
        assert_eq!(fen_to_str(5), "0.05");
        assert_eq!(ppm_to_str(300), "0.000300");
        assert_eq!(compute_tax_fen(123_456_789, 300), 37_037); // 1234567.89 × 0.0003 = 370.37
        assert_eq!(compute_tax_fen(2_000_000, 500), 1_000);
    }

    #[test]
    fn sqref() {
        assert_eq!(
            xlsx_patch::extend_sqref("D4:E4 G4:H4 M10:M30 I10:K30", 30, 33),
            "D4:E4 G4:H4 M10:M33 I10:K33"
        );
        assert_eq!(xlsx_patch::extend_sqref("Q1:Q30", 30, 30), "Q1:Q30");
    }

    #[test]
    fn rejects_property_transfer_without_sub_item() {
        let r = StampTaxReport {
            period_start: d("2026-10-01"),
            period_end: d("2026-10-31"),
            declare_type: DeclareType::Normal,
            lines: vec![line("101110200", "产权转移书据", None, 500, 100)],
        };
        assert!(matches!(validate(&r), Err(ReportError::Invalid(_))));
    }

    #[test]
    fn generates_readable_workbook() {
        let mut lines = Vec::new();
        for _ in 0..12 {
            lines.push(line("101110111", "买卖合同", None, 300, 123_456_789));
            lines.push(line(
                "101110200",
                "产权转移书据",
                Some(("101110200YH22001", "土地使用权出让书据")),
                500,
                2_000_000,
            ));
        }
        let report = StampTaxReport {
            period_start: d("2026-10-01"),
            period_end: d("2026-10-31"),
            declare_type: DeclareType::Normal,
            lines,
        };
        let out = std::env::temp_dir().join("stamp_tax_test.xlsx");
        let summary = generate(TEMPLATE, &report, &out).unwrap();
        assert_eq!(summary.row_count, 24);

        let mut wb: Xlsx<_> = open_workbook(&out).unwrap();
        let ws = wb.worksheet_range(MAIN_SHEET).unwrap();
        // calamine 的坐标是 0 基：(行-1, 列-1)
        assert_eq!(ws.get_value((3, 10)), Some(&Data::String("11|正常申报".into())));   // K4
        assert_eq!(ws.get_value((9, 1)), Some(&Data::String("1".into())));              // B10
        assert_eq!(ws.get_value((9, 6)), Some(&Data::String("101110111|买卖合同".into()))); // G10
        assert_eq!(ws.get_value((9, 15)), Some(&Data::Float(370.37)));                 // P10
        assert_eq!(
            ws.get_value((10, 7)),
            Some(&Data::String("101110200YH22001|土地使用权出让书据".into()))            // H11
        );
        assert_eq!(ws.get_value((32, 1)), Some(&Data::String("24".into())));            // B33

        let settlement = wb.worksheet_range(SETTLEMENT_SHEET).unwrap();
        // 示例数据（第 4 行起）已清除：最后一个非空单元格在第 3 行以内
        assert!(settlement.end().map_or(true, |(row, _)| row < 3));
    }
}
