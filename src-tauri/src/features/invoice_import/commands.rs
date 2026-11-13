// src-tauri/src/features/invoice_import/commands.rs
//
// 架构：
//   阶段1 — 多线程并行打开 Excel 读取各 sheet（每线程独立文件句柄，OS page cache 命中）
//   阶段2 — rayon 并行解析所有 sheet（CPU 密集，真正受益于多核）
//   阶段3 — 单线程串行插入 SQLite（I/O，SQLite 不支持并发写）

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};

use rusqlite::{Connection, Transaction};

use crate::error::{AppError, RowParseError};
use crate::models::dto::ImportRequest;
use crate::DbState;

use rayon::prelude::*;

use super::detail::{parse_detail_sheet, DetailParseResult};
use super::header::{parse_header_sheet, HeaderParseResult};
use super::models::{
    ImportSummary, ParsedAir, ParsedBuilding, ParsedCargo, ParsedPassenger, ParsedRailway,
    ParsedRealEstate, ParsedToll, SheetImportResult,
};
use super::repository::{
    insert_air_details, insert_building_details, insert_cargo_details, insert_details,
    insert_headers, insert_passenger_details, insert_railway_details, insert_real_estate_details,
    insert_toll_details,
};
use super::special::{
    ParseResult,
    parse_air_sheet, parse_building_sheet, parse_cargo_sheet, parse_passenger_sheet,
    parse_railway_sheet, parse_real_estate_sheet, parse_toll_sheet,
};

use crate::excel_reader::{read_xlsx, SheetData};
use crate::utils::usize_to_u32;

const SHEET_BASES: [&str; 9] = [
    "发票基础信息",
    "信息汇总表",
    "建筑服务",
    "不动产经营租赁服务",
    "旅客运输服务",
    "货物运输服务",
    "航空运输电子客票行程单",
    "铁路电子客票",
    "通行费数电发票",
];

/// 按后缀把 sheet 分批：无后缀为批 ""，"…1"、"…2" 分别为批 "1"、"2"。
/// 后缀只认纯数字；不属于任何约定名的 sheet 忽略。批次按后缀字符串升序（BTreeMap）。
fn split_batches(
    sheet_map: HashMap<String, SheetData>,
) -> BTreeMap<String, HashMap<String, SheetData>> {
    let mut batches: BTreeMap<String, HashMap<String, SheetData>> = BTreeMap::new();
    for (name, data) in sheet_map {
        let suffix = SHEET_BASES.iter().find_map(|b| {
            name.strip_prefix(b)
                .filter(|s| s.chars().all(|c| c.is_ascii_digit()))
        });
        if let Some(s) = suffix {
            batches.entry(s.to_string()).or_default().insert(name, data);
        }
    }
    batches
}

/// 取本批的某类 sheet（名字 = base + 批后缀）。
fn remove_sheet(
    sheets: &mut HashMap<String, SheetData>,
    base: &str,
    suffix: &str,
) -> Option<SheetData> {
    sheets.remove(&format!("{base}{suffix}"))
}

#[tauri::command]
#[specta::specta]
pub async fn import_invoice_excel(
    request: ImportRequest,
    db: tauri::State<'_, DbState>,
) -> Result<ImportSummary, String> {
    let file_path = request.file_path.clone();
    let conn_arc = Arc::clone(&db.conn);

    tokio::task::spawn_blocking(move || run_import(&file_path, conn_arc))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e: AppError| e.to_string())
}

/// 一批数据 = 同一后缀的所有 sheet（解析后）。
struct Batch {
    suffix: String,
    header: HeaderParseResult,
    summary: DetailParseResult,
    building: Option<ParseResult<ParsedBuilding>>,
    real_estate: Option<ParseResult<ParsedRealEstate>>,
    passenger: Option<ParseResult<ParsedPassenger>>,
    cargo: Option<ParseResult<ParsedCargo>>,
    air: Option<ParseResult<ParsedAir>>,
    railway: Option<ParseResult<ParsedRailway>>,
    toll: Option<ParseResult<ParsedToll>>,
}

fn parse_batch(
    suffix: String,
    mut sheets: HashMap<String, SheetData>,
    shared: &[String],
) -> Result<Batch, AppError> {
    let mut take = |base: &str| remove_sheet(&mut sheets, base, &suffix);
    let header_sheet = take("发票基础信息")
        .ok_or_else(|| AppError::SheetNotFound(format!("发票基础信息{suffix}")))?;
    let summary_sheet = take("信息汇总表")
        .ok_or_else(|| AppError::SheetNotFound(format!("信息汇总表{suffix}")))?;
    let building_sheet = take("建筑服务");
    let real_estate_sheet = take("不动产经营租赁服务");
    let passenger_sheet = take("旅客运输服务");
    let cargo_sheet = take("货物运输服务");
    let air_sheet = take("航空运输电子客票行程单");
    let railway_sheet = take("铁路电子客票");
    let toll_sheet = take("通行费数电发票");

    let (mut header, mut summary) = (None, None);
    let (mut building, mut real_estate, mut passenger, mut cargo) = (None, None, None, None);
    let (mut air, mut railway, mut toll) = (None, None, None);
    rayon::scope(|s| {
        s.spawn(|_| header = Some(parse_header_sheet(&header_sheet, shared)));
        s.spawn(|_| summary = Some(parse_detail_sheet(&summary_sheet, shared)));
        s.spawn(|_| building = building_sheet.as_ref().map(|r| parse_building_sheet(r, shared)));
        s.spawn(|_| real_estate = real_estate_sheet.as_ref().map(|r| parse_real_estate_sheet(r, shared)));
        s.spawn(|_| passenger = passenger_sheet.as_ref().map(|r| parse_passenger_sheet(r, shared)));
        s.spawn(|_| cargo = cargo_sheet.as_ref().map(|r| parse_cargo_sheet(r, shared)));
        s.spawn(|_| air = air_sheet.as_ref().map(|r| parse_air_sheet(r, shared)));
        s.spawn(|_| railway = railway_sheet.as_ref().map(|r| parse_railway_sheet(r, shared)));
        s.spawn(|_| toll = toll_sheet.as_ref().map(|r| parse_toll_sheet(r, shared)));
    });

    Ok(Batch {
        suffix,
        header: header.unwrap(),
        summary: summary.unwrap(),
        building,
        real_estate,
        passenger,
        cargo,
        air,
        railway,
        toll,
    })
}

/// 本批解析错误（带批后缀的 sheet 名）。
fn batch_parse_errors(b: &Batch) -> Vec<SheetImportResult> {
    let sfx = &b.suffix;
    let mut out = Vec::new();
    let mut add = |base: &str, errs: &[RowParseError]| {
        if !errs.is_empty() {
            out.push(sheet_result_from_errors(&format!("{base}{sfx}"), errs));
        }
    };
    add("发票基础信息", &b.header.errors);
    add("信息汇总表", &b.summary.errors);
    macro_rules! opt {
        ($f:expr, $n:expr) => {
            if let Some(r) = &$f {
                add($n, &r.errors);
            }
        };
    }
    opt!(b.building, "建筑服务");
    opt!(b.real_estate, "不动产经营租赁服务");
    opt!(b.passenger, "旅客运输服务");
    opt!(b.cargo, "货物运输服务");
    opt!(b.air, "航空运输电子客票行程单");
    opt!(b.railway, "铁路电子客票");
    opt!(b.toll, "通行费数电发票");
    out
}

struct BatchInsert {
    sheets: Vec<SheetImportResult>,
    header_inserted: usize,
    detail_inserted: usize,
}

/// 单批插入：先发票头，再明细；header_id_map 只属于本批。
fn insert_batch(
    tx: &Transaction,
    b: Batch,
    seller_cache: &mut HashMap<String, i64>,
) -> Result<BatchInsert, AppError> {
    let sfx = b.suffix.as_str();
    let header_name = format!("发票基础信息{sfx}");
    let header_result = insert_headers(tx, b.header.rows, seller_cache)?;
    if !header_result.errors.is_empty() {
        return Ok(BatchInsert {
            sheets: vec![sheet_result_from_errors(&header_name, &header_result.errors)],
            header_inserted: 0,
            detail_inserted: 0,
        });
    }
    let header_id_map = header_result.unq_map;
    let header_inserted = header_result.inserted;

    macro_rules! do_insert {
        ($insert_fn:expr, $rows:expr, $base:expr) => {{
            let name = format!("{}{}", $base, sfx);
            let mut lnc: HashMap<i64, u32> = HashMap::new();
            match $insert_fn(tx, $rows, &header_id_map, &mut lnc) {
                Ok(r) => (r.inserted, make_sheet_result(&name, true, r.inserted, r.errors)),
                Err(e) => (0, make_sheet_result_err(&name, true, e.to_string())),
            }
        }};
        ($insert_fn:expr, $parse_result:expr, $base:expr, optional) => {{
            match $parse_result {
                Some(r) => do_insert!($insert_fn, r.rows, $base),
                None => (0, make_sheet_result(&format!("{}{}", $base, sfx), false, 0, vec![])),
            }
        }};
    }

    let results = [
        do_insert!(insert_details, b.summary.rows, "信息汇总表"),
        do_insert!(insert_building_details, b.building, "建筑服务", optional),
        do_insert!(insert_real_estate_details, b.real_estate, "不动产经营租赁服务", optional),
        do_insert!(insert_passenger_details, b.passenger, "旅客运输服务", optional),
        do_insert!(insert_cargo_details, b.cargo, "货物运输服务", optional),
        do_insert!(insert_air_details, b.air, "航空运输电子客票行程单", optional),
        do_insert!(insert_railway_details, b.railway, "铁路电子客票", optional),
        do_insert!(insert_toll_details, b.toll, "通行费数电发票", optional),
    ];

    let detail_inserted = results.iter().map(|(n, _)| n).sum();
    let mut sheets = vec![SheetImportResult {
        sheet_name: header_name,
        recognized: true,
        inserted: usize_to_u32(header_inserted),
        error_count: 0,
        errors: vec![],
    }];
    sheets.extend(results.into_iter().map(|(_, s)| s));
    Ok(BatchInsert { sheets, header_inserted, detail_inserted })
}

fn run_import(file_path: &str, db_conn: Arc<Mutex<Connection>>) -> Result<ImportSummary, AppError> {
    let (sheet_map, shared) = read_xlsx(file_path)?;

    // Step 1: 分批（后缀相同为一批）；至少要有一批
    let batch_sheets = split_batches(sheet_map);
    if batch_sheets.is_empty() {
        return Err(AppError::SheetNotFound("发票基础信息".into()));
    }

    // Step 2: 各批并行解析（批内 sheet 也并行）；缺必需 sheet 直接报错
    let batches: Vec<Batch> = batch_sheets
        .into_par_iter()
        .map(|(sfx, sheets)| parse_batch(sfx, sheets, &shared))
        .collect::<Result<_, _>>()?;

    // Step 3: 任何一批解析有错 → 不开事务
    let parse_errors: Vec<SheetImportResult> =
        batches.iter().flat_map(batch_parse_errors).collect();
    if !parse_errors.is_empty() {
        return Ok(ImportSummary {
            sheets: parse_errors,
            committed: false,
            rollback_reason: Some("解析阶段发现错误，已回滚".to_string()),
            seller_count: 0,
            header_inserted: 0,
            detail_inserted: 0,
        });
    }

    // Step 4: 一个事务，按批串行插入（每批先头后明细）
    let mut conn_guard = db_conn.lock().unwrap();
    let tx = conn_guard.transaction()?;
    let mut seller_cache: HashMap<String, i64> = HashMap::new();
    let mut sheets = Vec::new();
    let (mut header_inserted, mut detail_inserted) = (0usize, 0usize);
    let mut has_errors = false;

    for b in batches {
        let r = insert_batch(&tx, b, &mut seller_cache)?;
        has_errors = r.sheets.iter().any(|s| s.error_count > 0);
        sheets.extend(r.sheets);
        header_inserted += r.header_inserted;
        detail_inserted += r.detail_inserted;
        if has_errors {
            break;
        }
    }

    if has_errors {
        tx.rollback()?;
        return Ok(ImportSummary {
            sheets,
            committed: false,
            rollback_reason: Some("插入阶段发现错误，已回滚".to_string()),
            seller_count: usize_to_u32(seller_cache.len()),
            header_inserted: 0,
            detail_inserted: 0,
        });
    }

    tx.commit()?;
    Ok(ImportSummary {
        sheets,
        committed: true,
        rollback_reason: None,
        seller_count: usize_to_u32(seller_cache.len()),
        header_inserted: usize_to_u32(header_inserted),
        detail_inserted: usize_to_u32(detail_inserted),
    })
}

// ─────────────────────────────────────────────────────────────
// 辅助函数
// ─────────────────────────────────────────────────────────────

fn sheet_result_from_errors(sheet_name: &str, errors: &[RowParseError]) -> SheetImportResult {
    SheetImportResult {
        sheet_name: sheet_name.to_string(),
        recognized: true,
        inserted: 0,
        error_count: usize_to_u32(errors.len()),
        errors: errors
            .iter()
            .map(|e| super::models::CellError {
                row: e.row,
                col: e.col.clone(),
                message: e.message.clone(),
            })
            .collect(),
    }
}

fn make_sheet_result(
    sheet_name: &str,
    recognized: bool,
    inserted: usize,
    errors: Vec<RowParseError>,
) -> SheetImportResult {
    SheetImportResult {
        sheet_name: sheet_name.to_string(),
        recognized,
        inserted: usize_to_u32(inserted),
        error_count: usize_to_u32(errors.len()),
        errors: errors
            .iter()
            .map(|e| super::models::CellError {
                row: e.row,
                col: e.col.clone(),
                message: e.message.clone(),
            })
            .collect(),
    }
}

fn make_sheet_result_err(sheet_name: &str, recognized: bool, message: String) -> SheetImportResult {
    SheetImportResult {
        sheet_name: sheet_name.to_string(),
        recognized,
        inserted: 0,
        error_count: 1,
        errors: vec![super::models::CellError {
            row: 0,
            col: "unknown".to_string(),
            message,
        }],
    }
}
