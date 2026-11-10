// 通用导出 Excel：后端按当前筛选条件重新查询并写文件，数据不经过 IPC。
// 前端只传：筛选条件 + 列定义（key/标题/类型）+ 保存路径（由 dialog 选好）。
// 列 key 与前端表格列一致，即查询结果序列化后的 camelCase 字段名。
use rust_xlsxwriter::{Format, Workbook, Worksheet};
use serde::Deserialize;
use serde_json::Value;
use specta::Type;
use std::collections::HashMap;

use crate::features::input_tax_transfer::{self as itt, HistoryView};
use crate::features::period_close::FilingRequest;
use crate::features::query::{repository as q, ItemOverviewFilter};
use crate::features::reversal_query::{self as rq, ReversalQueryFilter};
use crate::DbState;

#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ExportColumn {
    pub key: String,
    pub title: String,
    /// "money" = 分的文本，导出为元；"number" = 数值；其余 = 文本
    pub kind: Option<String>,
    /// 枚举/布尔的显示文案，键为值的文本形式（如 "1"、"true"）
    pub labels: Option<HashMap<String, String>>,
}

fn cell(ws: &mut Worksheet, r: u32, c: u16, v: &Value, col: &ExportColumn, money: &Format) {
    let text = match v {
        Value::Null => return,
        Value::String(s) => s.clone(),
        other => other.to_string(),
    };
    let text = col.labels.as_ref().and_then(|m| m.get(&text)).cloned().unwrap_or(text);
    let _ = match col.kind.as_deref() {
        Some("money") => match text.parse::<i64>() {
            Ok(n) => ws.write_number_with_format(r, c, n as f64 / 100.0, money).map(|_| ()),
            Err(_) => ws.write_string(r, c, &text).map(|_| ()),
        },
        Some("number") => match text.parse::<f64>() {
            Ok(n) => ws.write_number(r, c, n).map(|_| ()),
            Err(_) => ws.write_string(r, c, &text).map(|_| ()),
        },
        _ => ws.write_string(r, c, &text).map(|_| ()),
    };
}

/// 一个 sheet：行是 JSON 对象（serde 序列化结果）。
pub struct SheetData {
    pub name: String,
    pub columns: Vec<ExportColumn>,
    pub rows: Vec<Value>,
}

pub fn write_xlsx(path: &str, sheets: &[SheetData]) -> Result<(), String> {
    if !path.to_ascii_lowercase().ends_with(".xlsx") {
        return Err("导出文件必须是 .xlsx".into());
    }
    let (head, money) = (Format::new().set_bold(), Format::new().set_num_format("#,##0.00"));
    let mut wb = Workbook::new();
    for s in sheets {
        let ws = wb.add_worksheet();
        ws.set_name(&s.name).map_err(|e| e.to_string())?;
        ws.set_freeze_panes(1, 0).map_err(|e| e.to_string())?;
        for (c, col) in s.columns.iter().enumerate() {
            let c = c as u16;
            ws.write_string_with_format(0, c, &col.title, &head).map_err(|e| e.to_string())?;
            ws.set_column_width(c, (col.title.chars().count() as f64 * 2.0).max(12.0))
                .map_err(|e| e.to_string())?;
        }
        for (i, row) in s.rows.iter().enumerate() {
            for (c, col) in s.columns.iter().enumerate() {
                if let Some(v) = lookup(row, &col.key) {
                    cell(ws, i as u32 + 1, c as u16, v, col, &money);
                }
            }
        }
    }
    wb.save(path).map_err(|e| format!("写入 Excel 失败：{e}"))
}

/// 支持点路径，如 "voucher.voucherNo"
fn lookup<'a>(row: &'a Value, key: &str) -> Option<&'a Value> {
    key.split('.').try_fold(row, |v, k| v.get(k))
}

fn to_rows<T: serde::Serialize>(rows: &[T]) -> Result<Vec<Value>, String> {
    rows.iter().map(|r| serde_json::to_value(r).map_err(|e| e.to_string())).collect()
}

/// 综合查询：只导出单值列（item），AP/IC 等多值记录组不导出。
#[tauri::command]
#[specta::specta]
pub async fn export_item_overview(
    filter: ItemOverviewFilter,
    columns: Vec<ExportColumn>,
    path: String,
    db: tauri::State<'_, DbState>,
) -> Result<u32, String> {
    let rows = {
        let conn = db.conn.lock().unwrap();
        q::query_item_overview(&conn, &filter).map_err(|e| e.to_string())?.0
    };
    let rows: Vec<Value> = rows
        .into_iter()
        .map(|r| serde_json::to_value(r.item).map_err(|e| e.to_string()))
        .collect::<Result<_, _>>()?;
    let n = rows.len() as u32;
    write_xlsx(&path, &[SheetData { name: "综合查询".into(), columns, rows }])?;
    Ok(n)
}

/// 红冲查询：汇总 + 明细两个 sheet。蓝字/红字视角由 filter.view 决定，列定义由前端按视角传。
#[tauri::command]
#[specta::specta]
pub async fn export_reversal(
    filter: ReversalQueryFilter,
    summary_columns: Vec<ExportColumn>,
    detail_columns: Vec<ExportColumn>,
    path: String,
    db: tauri::State<'_, DbState>,
) -> Result<u32, String> {
    let r = {
        let conn = db.conn.lock().unwrap();
        rq::run(&conn, &filter).map_err(|e| e.to_string())?
    };
    let (s, d) = if filter.view.as_deref() == Some("red") {
        (to_rows(&r.red_summary)?, to_rows(&r.red_details)?)
    } else {
        (to_rows(&r.summary)?, to_rows(&r.details)?)
    };
    let n = s.len() as u32;
    write_xlsx(
        &path,
        &[
            SheetData { name: "汇总".into(), columns: summary_columns, rows: s },
            SheetData { name: "明细".into(), columns: detail_columns, rows: d },
        ],
    )?;
    Ok(n)
}

/// 进项转出：某纳税所属期的核对列表。
#[tauri::command]
#[specta::specta]
pub async fn export_input_tax_transfer(
    request: FilingRequest,
    columns: Vec<ExportColumn>,
    path: String,
    db: tauri::State<'_, DbState>,
) -> Result<u32, String> {
    let review = {
        let mut conn = db.conn.lock().map_err(|_| "数据库连接锁定失败".to_string())?;
        itt::load_review(&mut conn, &request, HistoryView::All)?
    };
    let rows = to_rows(&review.rows)?;
    let n = rows.len() as u32;
    write_xlsx(&path, &[SheetData { name: "进项转出".into(), columns, rows }])?;
    Ok(n)
}

/// 进项转出查询页：各纳税所属期的行合并成一个 sheet。
/// 每行补上纳税所属期 queryPeriod，批次和进项转出凭证信息在行自带的 confirm.* 里。
#[tauri::command]
#[specta::specta]
pub async fn export_input_tax_transfer_history(
    period_from: String,
    period_to: String,
    view: HistoryView,
    columns: Vec<ExportColumn>,
    path: String,
    db: tauri::State<'_, DbState>,
) -> Result<u32, String> {
    let periods = {
        let mut conn = db.conn.lock().map_err(|_| "数据库连接锁定失败".to_string())?;
        itt::load_history(&mut conn, &period_from, &period_to, view)?
    };
    let mut rows = Vec::new();
    for p in &periods {
        let r = &p.review;
        let period = if r.filing_from == r.filing_to {
            r.filing_from.clone()
        } else {
            format!("{}-{}", r.filing_from, r.filing_to)
        };
        for mut row in to_rows(&r.rows)? {
            row["queryPeriod"] = Value::String(period.clone());
            rows.push(row);
        }
    }
    let n = rows.len() as u32;
    write_xlsx(&path, &[SheetData { name: "进项转出".into(), columns, rows }])?;
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_money_and_rejects_bad_ext() {
        let col = |k: &str, kind: Option<&str>| ExportColumn {
            key: k.into(),
            title: k.into(),
            kind: kind.map(Into::into),
            labels: None,
        };
        let p = std::env::temp_dir().join("export_test.xlsx");
        let rows = vec![serde_json::json!({"a":"12345","b":null,"c":true})];
        let sheet = SheetData {
            name: "t".into(),
            columns: vec![col("a", Some("money")), col("b", None), col("c", None)],
            rows,
        };
        write_xlsx(p.to_str().unwrap(), &[sheet]).unwrap();
        assert!(p.metadata().unwrap().len() > 0);
        assert!(write_xlsx("x.csv", &[]).is_err());
        let v = serde_json::json!({"voucher": {"voucherNo": "V1"}});
        assert_eq!(lookup(&v, "voucher.voucherNo").unwrap(), "V1");
        assert!(lookup(&serde_json::json!({"voucher": null}), "voucher.voucherNo").is_none());
    }
}

// ───────────────── 交会计做账的清单 ─────────────────
//
// 批次确认后导出：只含该批次已确认的明细（补充批次只含补充明细，已锁定的旧批次不重复），
// 明细页 + 汇总页，会计据此做账；做完账后用户回填凭证信息并锁定。

use crate::features::period_close::{BatchState, ConfirmInfo};

/// 取当前批次已确认的行（用 JSON 值判断，进项转出与印花税的行共用）。
fn accounting_rows<T: serde::Serialize>(
    rows: &[T],
    state: BatchState,
    batch_no: u32,
    confirm_of: impl Fn(&T) -> Option<&ConfirmInfo>,
    counted_of: impl Fn(&T) -> bool,
) -> Result<Vec<Value>, String> {
    if state == BatchState::Open {
        return Err("本批次还没有确认，请先确认再导出做账清单".to_string());
    }
    let picked: Vec<&T> = rows
        .iter()
        .filter(|r| counted_of(r) && confirm_of(r).is_some_and(|c| c.same_run && c.batch_no == batch_no))
        .collect();
    if picked.is_empty() {
        return Err("本批次没有需要做账的明细".to_string());
    }
    picked
        .into_iter()
        .map(|r| serde_json::to_value(r).map_err(|e| e.to_string()))
        .collect()
}

fn text_col(key: &str, title: &str) -> ExportColumn {
    ExportColumn { key: key.into(), title: title.into(), kind: None, labels: None }
}
fn money_col(key: &str, title: &str) -> ExportColumn {
    ExportColumn { key: key.into(), title: title.into(), kind: Some("money".into()), labels: None }
}

/// 汇总页：按 group_key 分组，对 money_key 求和（分）。输出行 { group, count, amount }。
fn summarize_rows(rows: &[Value], group_key: &str, money_key: &str) -> Vec<Value> {
    let mut order: Vec<String> = Vec::new();
    let mut acc: HashMap<String, (i64, i64)> = HashMap::new();
    for r in rows {
        let g = lookup(r, group_key).and_then(Value::as_str).unwrap_or("（未分类）").to_string();
        let m = lookup(r, money_key).and_then(Value::as_str).and_then(|s| s.parse::<i64>().ok()).unwrap_or(0);
        let e = acc.entry(g.clone()).or_insert_with(|| {
            order.push(g.clone());
            (0, 0)
        });
        e.0 += 1;
        e.1 += m;
    }
    order.sort();
    let (n, total) = acc.values().fold((0, 0), |a, v| (a.0 + v.0, a.1 + v.1));
    let mut out: Vec<Value> = order
        .iter()
        .map(|g| serde_json::json!({"group": g, "count": acc[g].0, "amount": acc[g].1.to_string()}))
        .collect();
    out.push(serde_json::json!({"group": "合计", "count": n, "amount": total.to_string()}));
    out
}

fn summary_columns(group_title: &str, amount_title: &str) -> Vec<ExportColumn> {
    vec![
        text_col("group", group_title),
        ExportColumn { key: "count".into(), title: "笔数".into(), kind: Some("number".into()), labels: None },
        money_col("amount", amount_title),
    ]
}

/// 进项转出：导出当前批次交会计做账的清单。columns 由前端传（与核对表导出同一套列）。
#[tauri::command]
#[specta::specta]
pub async fn export_input_tax_transfer_accounting(
    request: FilingRequest,
    columns: Vec<ExportColumn>,
    path: String,
    db: tauri::State<'_, DbState>,
) -> Result<u32, String> {
    let review = {
        let mut conn = db.conn.lock().map_err(|_| "数据库连接锁定失败".to_string())?;
        itt::load_review(&mut conn, &request, HistoryView::All)?
    };
    let rows = accounting_rows(&review.rows, review.state, review.batch_no, |r| r.confirm.as_ref(), |r| r.counted)?;
    let n = rows.len() as u32;
    let summary = summarize_rows(&rows, "certPeriod", "finalTransferTax");
    write_xlsx(
        &path,
        &[
            SheetData { name: "转出明细".into(), columns, rows },
            SheetData { name: "汇总".into(), columns: summary_columns("认证期间", "转出税额"), rows: summary },
        ],
    )?;
    Ok(n)
}

/// 印花税：导出当前批次交会计做账的清单。汇总页按印花税类别。
#[tauri::command]
#[specta::specta]
pub async fn export_stamp_tax_accounting(
    request: FilingRequest,
    columns: Vec<ExportColumn>,
    path: String,
    db: tauri::State<'_, DbState>,
) -> Result<u32, String> {
    let review = {
        let mut conn = db.conn.lock().map_err(|_| "数据库连接锁定失败".to_string())?;
        crate::features::stamp_tax::load_review(&mut conn, &request, crate::features::stamp_tax::StampHistoryView::All)?
    };
    let rows = accounting_rows(&review.rows, review.state, review.batch_no, |r| r.confirm.as_ref(), |r| r.counted)?;
    let n = rows.len() as u32;
    let summary = summarize_rows(&rows, "stampTaxCategory", "stampTaxAmount");
    write_xlsx(
        &path,
        &[
            SheetData { name: "印花税明细".into(), columns, rows },
            SheetData { name: "汇总".into(), columns: summary_columns("印花税类别", "行级印花税额"), rows: summary },
        ],
    )?;
    Ok(n)
}

#[cfg(test)]
mod accounting_tests {
    use super::*;

    #[test]
    fn summary_groups_sorts_and_totals() {
        let rows = vec![
            serde_json::json!({"certPeriod": "202602", "finalTransferTax": "300"}),
            serde_json::json!({"certPeriod": "202601", "finalTransferTax": "100"}),
            serde_json::json!({"certPeriod": "202602", "finalTransferTax": "50"}),
        ];
        let s = summarize_rows(&rows, "certPeriod", "finalTransferTax");
        let got: Vec<(String, i64, String)> = s
            .iter()
            .map(|v| (v["group"].as_str().unwrap().into(), v["count"].as_i64().unwrap(), v["amount"].as_str().unwrap().into()))
            .collect();
        assert_eq!(
            got,
            vec![
                ("202601".into(), 1, "100".into()),
                ("202602".into(), 2, "350".into()),
                ("合计".into(), 3, "450".into())
            ]
        );
    }

    #[test]
    fn open_batch_cannot_export() {
        let none: Vec<i32> = vec![];
        let err = accounting_rows(&none, BatchState::Open, 1, |_| None, |_| true).unwrap_err();
        assert!(err.contains("先确认"));
    }
}
