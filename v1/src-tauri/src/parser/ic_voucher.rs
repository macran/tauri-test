// src-tauri/src/parser/ic_voucher.rs
//
// 职责：
//   1. 读取 IC 凭证 Excel 第一个 Sheet
//   2. 动态定位列索引
//   3. 处理发票号码向上继承逻辑
//   4. 按 (LE + 发票号码) 汇总税额和价税合计，用于后续校验
//   5. 返回 ParsedIcVoucherRow 列表和解析错误列表

use calamine::{open_workbook, Data, Reader, Xlsx};

use crate::error::AppError;
use crate::models::dto::IcVoucherImportError;
use crate::utils::{
    c_is_empty_row, cell_to_f64, cell_to_string, excel_date_to_string, yuan_to_fen,
};

// ─────────────────────────────────────────────────────────────
// 列名常量
// ─────────────────────────────────────────────────────────────
const COL_LE: &str = "LE";
const COL_INV_CODE: &str = "发票代码";
const COL_INV_NUM: &str = "发票号码";
const COL_BILLING: &str = "Billing";
const COL_DOCUMENT_NO: &str = "Document Number";
const COL_POSTED_DATE: &str = "Posting Date";
const COL_INV_DATE: &str = "开票日期";
const COL_AMOUNT: &str = "金额";
const COL_TAX_RATE: &str = "税率";
const COL_TAX: &str = "税额";
const COL_TOTAL: &str = "税价合计";
const COL_ORIGINAL_PO: &str = "Original PO";

// ─────────────────────────────────────────────────────────────
// 列索引映射
// ─────────────────────────────────────────────────────────────
struct ColIndex {
    le: usize,
    inv_code: Option<usize>,
    inv_num: usize,
    billing: usize,
    document_no: usize,
    posted_date: usize,
    inv_date: usize,
    #[allow(dead_code)]
    amount: usize,
    #[allow(dead_code)]
    tax_rate: usize,
    tax: usize,
    total: usize,
    original_po: Option<usize>,
}

fn build_col_index(header_row: &[Data]) -> Result<ColIndex, String> {
    let mut map: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for (i, cell) in header_row.iter().enumerate() {
        if let Some(s) = cell_to_string(cell) {
            map.insert(s.trim().to_string(), i);
        }
    }

    macro_rules! req {
        ($name:expr) => {
            map.get($name)
                .copied()
                .ok_or_else(|| format!("表头缺少必填列「{}」", $name))?
        };
    }
    macro_rules! opt {
        ($name:expr) => {
            map.get($name).copied()
        };
    }

    Ok(ColIndex {
        le: req!(COL_LE),
        inv_code: opt!(COL_INV_CODE),
        inv_num: req!(COL_INV_NUM),
        billing: req!(COL_BILLING),
        document_no: req!(COL_DOCUMENT_NO),
        posted_date: req!(COL_POSTED_DATE),
        inv_date: req!(COL_INV_DATE),
        amount: req!(COL_AMOUNT),
        tax_rate: req!(COL_TAX_RATE),
        tax: req!(COL_TAX),
        total: req!(COL_TOTAL),
        original_po: opt!(COL_ORIGINAL_PO),
    })
}

// ─────────────────────────────────────────────────────────────
// 解析后的单行数据
// ─────────────────────────────────────────────────────────────
#[derive(Debug, Clone)]
pub struct ParsedIcVoucherRow {
    pub excel_row: u32,
    pub legal_entity: String,
    pub inv_code: Option<String>,
    pub inv_num: String, // 原始发票号码（继承后）
    pub billing: Option<String>,
    pub document_no: String,
    pub posted_date: String,
    pub original_po: Option<String>,
    /// 该行税额（分），用于汇总校验
    pub tax_fen: i64,
    /// 该行价税合计（分），用于汇总校验
    pub total_fen: i64,
}

// ─────────────────────────────────────────────────────────────
// 汇总 key：(LE, 发票号码)
// ─────────────────────────────────────────────────────────────
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct InvGroupKey {
    pub legal_entity: String,
    pub inv_num: String,
    pub inv_code: Option<String>,
}

// ─────────────────────────────────────────────────────────────
// 汇总结果：用于和 header 校验
// ─────────────────────────────────────────────────────────────
#[derive(Debug, Clone)]
pub struct InvGroupSummary {
    pub key: InvGroupKey,
    /// 该组所有行的税额之和（分）
    pub total_tax_fen: i64,
    /// 该组所有行的价税合计之和（分）
    pub total_amount_fen: i64,
    /// 该组包含的所有行（用于入库）
    pub rows: Vec<ParsedIcVoucherRow>,
}

// ─────────────────────────────────────────────────────────────
// 公开入口
// ─────────────────────────────────────────────────────────────
pub fn parse_ic_voucher_excel(
    file_path: &str,
) -> Result<
    (
        Vec<InvGroupSummary>,      // 按发票分组后的数据（含校验用汇总）
        Vec<IcVoucherImportError>, // 解析阶段错误
    ),
    AppError,
> {
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

    let col_idx = build_col_index(&rows[0]).map_err(AppError::ExcelParse)?;

    let mut parsed_rows: Vec<ParsedIcVoucherRow> = Vec::new();
    let mut errors: Vec<IcVoucherImportError> = Vec::new();

    // 用于发票号码向上继承
    let mut last_inv_num: Option<String> = None;
    let mut last_inv_code: Option<String> = None;

    for (i, row) in rows.iter().enumerate().skip(1) {
        let excel_row = (i + 1) as u32;

        if c_is_empty_row(row) {
            continue;
        }

        match parse_row(row, excel_row, &col_idx, &last_inv_num, &last_inv_code) {
            Ok(parsed) => {
                // 更新继承状态：只有本行有值时才更新
                last_inv_num = Some(parsed.inv_num.clone());
                last_inv_code = parsed.inv_code.clone();
                parsed_rows.push(parsed);
            }
            Err(err) => {
                errors.push(err);
                // 解析失败的行不更新继承状态，避免错误传播
            }
        }
    }

    // ── 按 (LE + 发票号码 + 发票代码) 分组汇总 ──────────────
    let groups = group_by_invoice(parsed_rows);

    Ok((groups, errors))
}

// ─────────────────────────────────────────────────────────────
// 单行解析
// ─────────────────────────────────────────────────────────────
fn parse_row(
    row: &[Data],
    excel_row: u32,
    idx: &ColIndex,
    last_inv_num: &Option<String>,
    last_inv_code: &Option<String>,
) -> Result<ParsedIcVoucherRow, IcVoucherImportError> {
    let opt_str = |col: Option<usize>| -> Option<String> {
        col.and_then(|i| row.get(i)).and_then(|c| cell_to_string(c))
    };

    let req_str = |col: usize, col_name: &str| -> Result<String, IcVoucherImportError> {
        row.get(col)
            .and_then(|c| cell_to_string(c))
            .ok_or_else(|| make_err(excel_row, "", "", &format!("必填字段「{}」为空", col_name)))
    };

    // 金额列：空白视为 0（同一发票的后续凭证行可不填金额）；非空但无法解析才报错
    let req_amount = |col: usize, col_name: &str| -> Result<i64, IcVoucherImportError> {
        let Some(cell) = row.get(col) else {
            return Ok(0);
        };
        if cell_to_string(cell).is_none() {
            return Ok(0);
        }
        cell_to_f64(cell)
            .or_else(|| {
                cell_to_string(cell).and_then(|s| s.replace(',', "").trim().parse::<f64>().ok())
            })
            .map(yuan_to_fen)
            .ok_or_else(|| {
                make_err(
                    excel_row,
                    "",
                    "",
                    &format!("「{}」无法解析为数字", col_name),
                )
            })
    };

    // ── LE ───────────────────────────────────────────────────
    let legal_entity = req_str(idx.le, COL_LE)?;

    // ── 发票号码（支持向上继承）──────────────────────────────
    let raw_inv_num = row.get(idx.inv_num).and_then(|c| cell_to_string(c));
    let inv_num = match raw_inv_num {
        Some(s) if !s.trim().is_empty() => s.trim().to_string(),
        _ => {
            // 本行为空，继承上一行
            match last_inv_num {
                Some(prev) => prev.clone(),
                None => {
                    return Err(make_err(
                        excel_row,
                        &legal_entity,
                        "",
                        "「发票号码」为空且无上一行可继承",
                    ));
                }
            }
        }
    };

    // ── 发票代码（可选，继承逻辑同发票号码）─────────────────
    let raw_inv_code = opt_str(idx.inv_code);
    let inv_code = match raw_inv_code {
        Some(s) if !s.trim().is_empty() => Some(s.trim().to_string()),
        _ => last_inv_code.clone(),
    };

    // ── Document Number ──────────────────────────────────────
    let document_no = req_str(idx.document_no, COL_DOCUMENT_NO).map_err(|mut e| {
        e.inv_num = inv_num.clone();
        e
    })?;

    // ── Billing（可选）──────────────────────────────────────
    let billing = opt_str(Some(idx.billing));

    // ── Original PO（可选）──────────────────────────────────
    let original_po = opt_str(idx.original_po);

    // ── Posting Date ─────────────────────────────────────────
    let posted_date = match excel_date_to_string(row.get(idx.posted_date)) {
        Ok(d) => d,
        Err(reason) => {
            return Err(make_err(
                excel_row,
                &legal_entity,
                &inv_num,
                &format!("「{}」解析失败：{}", COL_POSTED_DATE, reason),
            ));
        }
    };

    // ── 开票日期（解析但不存库，仅校验用）───────────────────
    // 后续凭证行可为空，空白时跳过
    let inv_date_blank = row
        .get(idx.inv_date)
        .map_or(true, |c| cell_to_string(c).is_none());
    if !inv_date_blank {
        if let Err(reason) = excel_date_to_string(row.get(idx.inv_date)) {
            return Err(make_err(
                excel_row,
                &legal_entity,
                &inv_num,
                &format!("「{}」解析失败：{}", COL_INV_DATE, reason),
            ));
        }
    }

    // ── 税额、价税合计（元 → 分）────────────────────────────
    let tax_fen = req_amount(idx.tax, COL_TAX).map_err(|mut e| {
        e.legal_entity = legal_entity.clone();
        e.inv_num = inv_num.clone();
        e
    })?;

    let total_fen = req_amount(idx.total, COL_TOTAL).map_err(|mut e| {
        e.legal_entity = legal_entity.clone();
        e.inv_num = inv_num.clone();
        e
    })?;

    Ok(ParsedIcVoucherRow {
        excel_row,
        legal_entity,
        inv_code,
        inv_num,
        billing,
        document_no,
        posted_date,
        original_po,
        tax_fen,
        total_fen,
    })
}

// ─────────────────────────────────────────────────────────────
// 按 (LE + 发票号码 + 发票代码) 分组汇总
// 保持 Excel 中的原始顺序
// ─────────────────────────────────────────────────────────────
fn group_by_invoice(rows: Vec<ParsedIcVoucherRow>) -> Vec<InvGroupSummary> {
    let mut groups: Vec<InvGroupSummary> = Vec::new();
    let mut key_to_idx: std::collections::HashMap<InvGroupKey, usize> =
        std::collections::HashMap::new();

    for row in rows {
        let key = InvGroupKey {
            legal_entity: row.legal_entity.clone(),
            inv_num: row.inv_num.clone(),
            inv_code: row.inv_code.clone(),
        };

        if let Some(&idx) = key_to_idx.get(&key) {
            // 已有分组，累加
            groups[idx].total_tax_fen += row.tax_fen;
            groups[idx].total_amount_fen += row.total_fen;
            groups[idx].rows.push(row);
        } else {
            // 新分组
            let idx = groups.len();
            key_to_idx.insert(key.clone(), idx);
            groups.push(InvGroupSummary {
                total_tax_fen: row.tax_fen,
                total_amount_fen: row.total_fen,
                key,
                rows: vec![row],
            });
        }
    }

    groups
}

// ─────────────────────────────────────────────────────────────
// 工具
// ─────────────────────────────────────────────────────────────
fn make_err(row: u32, legal_entity: &str, inv_num: &str, message: &str) -> IcVoucherImportError {
    IcVoucherImportError {
        row,
        legal_entity: legal_entity.to_string(),
        inv_num: inv_num.to_string(),
        document_no: String::new(),
        error_kind: "解析错误".to_string(),
        message: message.to_string(),
    }
}
