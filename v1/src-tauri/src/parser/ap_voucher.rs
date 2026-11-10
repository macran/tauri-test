// src-tauri/src/parser/ap_voucher.rs
//
// 职责：
//   1. 读取 AP 凭证 Excel（第一个 Sheet）
//   2. 根据列名动态定位列索引（列顺序不固定）
//   3. 解析每行数据为 ParsedApVoucherRow
//   4. 返回解析结果和解析阶段错误
use calamine::{open_workbook, Data, Reader, Xlsx};
use std::str::FromStr;

use crate::error::AppError;
use crate::models::dto::ApVoucherImportError;
use crate::models::enums::{ApInvType, TaxCertified};
use crate::utils::{
    c_is_empty_row, cell_to_f64, cell_to_string, excel_date_to_string, yuan_to_fen,
};

// ─────────────────────────────────────────────────────────────
// 解析后的单行数据（解析阶段产物，尚未写库）
// ─────────────────────────────────────────────────────────────
#[derive(Debug, Clone)]
pub struct ParsedApVoucherRow {
    pub excel_row: u32,
    pub barcode: Option<String>,
    pub inv_code: Option<String>, // 发票代码
    pub inv_num: String,          // 发票号码（必填；收据/通知单也会有数据编码或占位文本）
    pub entry_no: Option<String>, // 入账号码
    pub amount: i64,              // 不含税金额（分）
    pub tax_rate: Option<String>, // 税率（原始字符串）
    pub tax: i64,                 // 税额（分）
    pub total: i64,               // 含税金额（分）
    pub inv_type: ApInvType,
    pub po_no: Option<String>,
    pub vendor_code: String,
    pub vendor_name: String,
    pub voucher_no: String,           // 凭证号，业务上保证非空
    pub invoice_date: Option<String>, // 开票日期
    pub entry_date: Option<String>,   // 入账日期
    pub remark: Option<String>,
    pub tax_certified: TaxCertified,
}

// ─────────────────────────────────────────────────────────────
// 列名常量（Excel 表头中的中文列名）
// ─────────────────────────────────────────────────────────────
const COL_BARCODE: &str = "Barcode";
const COL_INV_CODE: &str = "发票代码";
const COL_INV_NUM: &str = "发票号码";
const COL_ENTRY_NO: &str = "入账号码";
const COL_AMOUNT: &str = "不含税金额";
const COL_INV_DATE: &str = "发票日期";
const COL_TAX_RATE: &str = "税率";
const COL_TAX: &str = "税额";
const COL_TOTAL: &str = "含税金额";
const COL_INV_TYPE: &str = "发票类型";
const COL_PO_NO: &str = "PO No.";
const COL_VENDOR_CODE: &str = "Vendor Code";
const COL_VENDOR_NAME: &str = "Vendor Name";
const COL_VOUCHER_NO: &str = "凭证号";
const COL_ENTRY_DATE: &str = "入账日期";
const COL_REMARK: &str = "备注";
const COL_TAX_CERTIFIED: &str = "税务认证";

// ─────────────────────────────────────────────────────────────
// 列索引映射
// ─────────────────────────────────────────────────────────────
struct ColIndex {
    barcode: Option<usize>,
    inv_code: Option<usize>,
    inv_num: usize,
    entry_no: Option<usize>,
    amount: usize,
    tax_rate: Option<usize>,
    tax: usize,
    total: usize,
    inv_type: usize,
    po_no: Option<usize>,
    vendor_code: usize,
    vendor_name: usize,
    voucher_no: usize,
    invoice_date: Option<usize>,
    entry_date: Option<usize>,
    remark: Option<usize>,
    tax_certified: usize,
}

/// 从表头行构建列索引映射。
/// 必填列缺失时返回 Err。
fn build_col_index(header_row: &[Data]) -> Result<ColIndex, String> {
    let mut map: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for (i, cell) in header_row.iter().enumerate() {
        if let Some(s) = cell_to_string(cell) {
            map.insert(s.trim().to_string(), i);
        }
    }

    // 必填列
    macro_rules! req {
        ($name:expr) => {
            map.get($name)
                .copied()
                .ok_or_else(|| format!("表头缺少必填列「{}」", $name))?
        };
    }
    // 可选列
    macro_rules! opt {
        ($name:expr) => {
            map.get($name).copied()
        };
    }

    Ok(ColIndex {
        barcode: opt!(COL_BARCODE),
        inv_code: opt!(COL_INV_CODE),
        inv_num: req!(COL_INV_NUM),
        entry_no: opt!(COL_ENTRY_NO),
        amount: req!(COL_AMOUNT),
        tax_rate: opt!(COL_TAX_RATE),
        tax: req!(COL_TAX),
        total: req!(COL_TOTAL),
        inv_type: req!(COL_INV_TYPE),
        po_no: opt!(COL_PO_NO),
        vendor_code: req!(COL_VENDOR_CODE),
        vendor_name: req!(COL_VENDOR_NAME),
        voucher_no: req!(COL_VOUCHER_NO),
        invoice_date: opt!(COL_INV_DATE),
        entry_date: opt!(COL_ENTRY_DATE),
        remark: opt!(COL_REMARK),
        tax_certified: req!(COL_TAX_CERTIFIED),
    })
}

// ─────────────────────────────────────────────────────────────
// 公开入口
// ─────────────────────────────────────────────────────────────
pub fn parse_ap_voucher_excel(
    file_path: &str,
) -> Result<(Vec<ParsedApVoucherRow>, Vec<ApVoucherImportError>), AppError> {
    let mut workbook: Xlsx<_> = open_workbook(file_path)?;

    let sheet_name = workbook
        .sheet_names()
        .first()
        .cloned()
        .ok_or_else(|| AppError::SheetNotFound("第一个Sheet".to_string()))?;

    let sheet = workbook
        .worksheet_range(&sheet_name)
        .map_err(|_| AppError::SheetNotFound(sheet_name.clone()))?;

    let rows: Vec<Vec<Data>> = sheet.rows().map(|r| r.to_vec()).collect();

    if rows.is_empty() {
        return Err(AppError::ExcelParse("Excel 文件为空".to_string()));
    }

    // ── 第一行是表头 ─────────────────────────────────────────
    let col_idx = build_col_index(&rows[0]).map_err(|e| AppError::ExcelParse(e))?;

    let mut parsed_rows: Vec<ParsedApVoucherRow> = Vec::new();
    let mut errors: Vec<ApVoucherImportError> = Vec::new();

    for (idx, row) in rows.iter().enumerate().skip(1) {
        let excel_row = (idx + 1) as u32;

        // 跳过空行
        if c_is_empty_row(row) {
            continue;
        }

        match parse_row(row, excel_row, &col_idx) {
            Ok(parsed) => parsed_rows.push(parsed),
            Err(err) => errors.push(err),
        }
    }

    Ok((parsed_rows, errors))
}

// ─────────────────────────────────────────────────────────────
// 单行解析
// ─────────────────────────────────────────────────────────────
fn parse_row(
    row: &[Data],
    excel_row: u32,
    idx: &ColIndex,
) -> Result<ParsedApVoucherRow, ApVoucherImportError> {
    // 辅助闭包：从行中取可选字符串
    let opt_str = |col: Option<usize>| -> Option<String> {
        col.and_then(|i| row.get(i)).and_then(|c| cell_to_string(c))
    };

    // 辅助闭包：从行中取必填字符串
    let req_str = |col: usize, col_name: &str| -> Result<String, ApVoucherImportError> {
        row.get(col).and_then(|c| cell_to_string(c)).ok_or_else(|| {
            make_err(
                excel_row,
                None,
                None,
                "必填字段为空",
                &format!("「{}」不能为空", col_name),
            )
        })
    };

    // 辅助闭包：从行中取金额（元 → 分）
    let req_amount = |col: usize, col_name: &str| -> Result<i64, ApVoucherImportError> {
        let s = row.get(col).and_then(|c| cell_to_f64(c)).ok_or_else(|| {
            make_err(
                excel_row,
                None,
                None,
                "金额解析失败",
                &format!("「{}」无法解析为数字", col_name),
            )
        })?;
        Ok(yuan_to_fen(s))
    };

    // ── 提取各字段 ───────────────────────────────────────────
    let barcode = opt_str(idx.barcode);
    let inv_code = opt_str(idx.inv_code);

    let inv_num = req_str(idx.inv_num, COL_INV_NUM).map_err(|mut e| {
        e.barcode = barcode.clone();
        e
    })?;
    let entry_no = opt_str(idx.entry_no);
    let tax_rate = opt_str(idx.tax_rate);
    let po_no = opt_str(idx.po_no);
    let voucher_no = req_str(idx.voucher_no, COL_VOUCHER_NO).map_err(|mut e| {
        e.barcode = barcode.clone();
        e.inv_num = Some(inv_num.clone());
        e
    })?;
    let invoice_date = opt_date(
        idx.invoice_date.and_then(|i| row.get(i)),
        excel_row,
        COL_INV_DATE,
        barcode.clone(),
        Some(inv_num.clone()),
    )?;
    let entry_date = opt_date(
        idx.entry_date.and_then(|i| row.get(i)),
        excel_row,
        COL_ENTRY_DATE,
        barcode.clone(),
        Some(inv_num.clone()),
    )?;
    let remark = opt_str(idx.remark);

    let amount = req_amount(idx.amount, COL_AMOUNT).map_err(|mut e| {
        e.barcode = barcode.clone();
        e.inv_num = Some(inv_num.clone());
        e
    })?;
    let tax = req_amount(idx.tax, COL_TAX).map_err(|mut e| {
        e.barcode = barcode.clone();
        e.inv_num = Some(inv_num.clone());
        e
    })?;
    let total = req_amount(idx.total, COL_TOTAL).map_err(|mut e| {
        e.barcode = barcode.clone();
        e.inv_num = Some(inv_num.clone());
        e
    })?;

    let vendor_code = req_str(idx.vendor_code, COL_VENDOR_CODE).map_err(|mut e| {
        e.barcode = barcode.clone();
        e.inv_num = Some(inv_num.clone());
        e
    })?;
    let vendor_name = req_str(idx.vendor_name, COL_VENDOR_NAME).map_err(|mut e| {
        e.barcode = barcode.clone();
        e.inv_num = Some(inv_num.clone());
        e
    })?;

    // ── 发票类型枚举解析 ─────────────────────────────────────
    let inv_type_str = req_str(idx.inv_type, COL_INV_TYPE).map_err(|mut e| {
        e.barcode = barcode.clone();
        e.inv_num = Some(inv_num.clone());
        e
    })?;
    let inv_type = ApInvType::from_str(&inv_type_str).map_err(|_| {
        make_err(
            excel_row,
            barcode.clone(),
            Some(inv_num.clone()),
            "发票类型非法",
            &format!(
                "「发票类型」值「{}」不合法，合法值为：增票、普票、红票、收据、票据、结算单",
                inv_type_str
            ),
        )
    })?;

    // ── 旧式纸质发票（非 20 位数电号）需要增票/普票时，发票代码不能为空 ──
    if inv_type.requires_header() && inv_num.len() != 20 && inv_code.as_deref().unwrap_or("").trim().is_empty() {
        return Err(make_err(
            excel_row,
            barcode.clone(),
            Some(inv_num.clone()),
            "必填字段为空",
            "旧式发票（非数电票）缺少「发票代码」，无法匹配发票",
        ));
    }

    // ── 税务认证枚举解析 ─────────────────────────────────────
    let tax_cert_str = req_str(idx.tax_certified, COL_TAX_CERTIFIED).map_err(|mut e| {
        e.barcode = barcode.clone();
        e.inv_num = Some(inv_num.clone());
        e
    })?;
    let tax_certified = TaxCertified::from_str(&tax_cert_str).map_err(|_| {
        make_err(
            excel_row,
            barcode.clone(),
            Some(inv_num.clone()),
            "税务认证非法",
            &format!("「税务认证」值「{}」不合法，合法值为：是、否", tax_cert_str),
        )
    })?;

    Ok(ParsedApVoucherRow {
        excel_row,
        barcode,
        inv_code,
        inv_num,
        entry_no,
        amount,
        tax_rate,
        tax,
        total,
        inv_type,
        po_no,
        vendor_code,
        vendor_name,
        voucher_no,
        invoice_date,
        entry_date,
        remark,
        tax_certified,
    })
}

// ─────────────────────────────────────────────────────────────
// 工具函数
// ─────────────────────────────────────────────────────────────

fn make_err(
    row: u32,
    barcode: Option<String>,
    inv_num: Option<String>,
    kind: &str,
    message: &str,
) -> ApVoucherImportError {
    ApVoucherImportError {
        row,
        barcode,
        inv_num,
        error_kind: kind.to_string(),
        message: message.to_string(),
    }
}

fn opt_date(
    cell: Option<&Data>,
    excel_row: u32,
    col_name: &str,
    barcode: Option<String>,
    inv_num: Option<String>,
) -> Result<Option<String>, ApVoucherImportError> {
    match cell {
        None | Some(Data::Empty) => Ok(None),
        Some(c) => excel_date_to_string(Some(c)).map(Some).map_err(|reason| {
            make_err(
                excel_row,
                barcode,
                inv_num,
                "日期解析失败",
                &format!("「{}」{}", col_name, reason),
            )
        }),
    }
}
