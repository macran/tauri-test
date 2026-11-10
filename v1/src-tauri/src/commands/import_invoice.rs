// src-tauri/src/commands/import_invoice.rs
//
// 架构：
//   阶段1 — 多线程并行打开 Excel 读取各 sheet（每线程独立文件句柄，OS page cache 命中）
//   阶段2 — rayon 并行解析所有 sheet（CPU 密集，真正受益于多核）
//   阶段3 — 单线程串行插入 SQLite（I/O，SQLite 不支持并发写）

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use rusqlite::Connection;

use crate::db::operations::{
    insert_air_details, insert_building_details, insert_cargo_details, insert_details,
    insert_headers, insert_passenger_details, insert_railway_details, insert_real_estate_details,
    insert_toll_details,
};
use crate::error::{AppError, RowParseError};
use crate::models::dto::{ImportRequest, ImportSummary, SheetImportResult};
use crate::parser::{
    detail::parse_detail_sheet,
    header::parse_header_sheet,
    special::{
        parse_air_sheet, parse_building_sheet, parse_cargo_sheet, parse_passenger_sheet,
        parse_railway_sheet, parse_real_estate_sheet, parse_toll_sheet,
    },
};

use crate::excel_reader::{read_xlsx, SheetData};
use crate::utils::usize_to_u32;
use crate::DbState;

/// 按约定名匹配 sheet：优先精确匹配 `base`，找不到则尝试 `base` + "1"。
/// 同一份文件中，同一类内容只会以这两种名字之一出现，不会同时存在。
fn remove_sheet_variant(
    sheet_map: &mut HashMap<String, SheetData>,
    base: &str,
) -> Option<SheetData> {
    sheet_map
        .remove(base)
        .or_else(|| sheet_map.remove(&format!("{base}1")))
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

fn run_import(file_path: &str, db_conn: Arc<Mutex<Connection>>) -> Result<ImportSummary, AppError> {
    // Step 1: 用 zip+quick-xml 读取所有 sheet（sharedStrings 只解析一次，各 sheet 并行解析）
    let t_read = std::time::Instant::now();

    // const ALL_SHEETS: &[&str] = &[
    //     "发票基础信息",
    //     "信息汇总表",
    //     "建筑服务",
    //     "不动产经营租赁服务",
    //     "旅客运输服务",
    //     "货物运输服务",
    //     "航空运输电子客票行程单",
    //     "铁路电子客票",
    //     "通行费数电发票",
    // ];

    let (mut sheet_map, shared) = read_xlsx(file_path)?;
    eprintln!("[TIMING] openworkbook: {:?}", t_read.elapsed());

    let t_read1 = std::time::Instant::now();
    let header_sheet = remove_sheet_variant(&mut sheet_map, "发票基础信息")
        .ok_or_else(|| AppError::SheetNotFound("发票基础信息".into()))?;
    let summary_sheet = remove_sheet_variant(&mut sheet_map, "信息汇总表")
        .ok_or_else(|| AppError::SheetNotFound("信息汇总表".into()))?;

    let building_sheet = remove_sheet_variant(&mut sheet_map, "建筑服务");
    let real_estate_sheet = remove_sheet_variant(&mut sheet_map, "不动产经营租赁服务");
    let passenger_sheet = remove_sheet_variant(&mut sheet_map, "旅客运输服务");
    let cargo_sheet = remove_sheet_variant(&mut sheet_map, "货物运输服务");
    let air_sheet = remove_sheet_variant(&mut sheet_map, "航空运输电子客票行程单");
    let railway_sheet = remove_sheet_variant(&mut sheet_map, "铁路电子客票");
    let toll_sheet = remove_sheet_variant(&mut sheet_map, "通行费数电发票");

    eprintln!("[TIMING] readworksheetrange: {:?}", t_read1.elapsed());
    // ── Step 2: 串行解析发票头（必须先完成，后续明细依赖 header_id_map）──

    let t_read2 = std::time::Instant::now();
    let header_parse = parse_header_sheet(&header_sheet, &shared);

    if !header_parse.errors.is_empty() {
        return Ok(ImportSummary {
            batch_no: 0,
            sheets: vec![sheet_result_from_errors(
                "发票基础信息",
                &header_parse.errors,
            )],
            committed: false,
            rollback_reason: Some(format!(
                "发票基础信息解析失败：{} 行错误",
                header_parse.errors.len()
            )),
            seller_count: 0,
            header_inserted: 0,
            detail_inserted: 0,
        });
    }
    eprintln!("[TIMING] parse_header_sheet: {:?}", t_read2.elapsed());
    // ── Step 3: rayon 并行解析所有明细 sheet ─────────────────────────────
    let t_read3 = std::time::Instant::now();
    let (
        summary_parse,
        (
            building_parse,
            (
                real_estate_parse,
                (passenger_parse, (cargo_parse, (air_parse, (railway_parse, toll_parse)))),
            ),
        ),
    ) = rayon::join(
        || parse_detail_sheet(&summary_sheet, &shared),
        || {
            rayon::join(
                || {
                    building_sheet
                        .as_ref()
                        .map(|r| parse_building_sheet(r, &shared))
                },
                || {
                    rayon::join(
                        || {
                            real_estate_sheet
                                .as_ref()
                                .map(|r| parse_real_estate_sheet(r, &shared))
                        },
                        || {
                            rayon::join(
                                || {
                                    passenger_sheet
                                        .as_ref()
                                        .map(|r| parse_passenger_sheet(r, &shared))
                                },
                                || {
                                    rayon::join(
                                        || {
                                            cargo_sheet
                                                .as_ref()
                                                .map(|r| parse_cargo_sheet(r, &shared))
                                        },
                                        || {
                                            rayon::join(
                                                || {
                                                    air_sheet
                                                        .as_ref()
                                                        .map(|r| parse_air_sheet(r, &shared))
                                                },
                                                || {
                                                    rayon::join(
                                                        || {
                                                            railway_sheet.as_ref().map(|r| {
                                                                parse_railway_sheet(r, &shared)
                                                            })
                                                        },
                                                        || {
                                                            toll_sheet.as_ref().map(|r| {
                                                                parse_toll_sheet(r, &shared)
                                                            })
                                                        },
                                                    )
                                                },
                                            )
                                        },
                                    )
                                },
                            )
                        },
                    )
                },
            )
        },
    );

    // ── Step 4: 检查所有明细 sheet 解析错误，有错误直接返回，不开事务 ────
    let mut parse_error_sheets: Vec<SheetImportResult> = Vec::new();

    if !summary_parse.errors.is_empty() {
        parse_error_sheets.push(sheet_result_from_errors(
            "信息汇总表",
            &summary_parse.errors,
        ));
    }
    macro_rules! check_opt_errors {
        ($result:expr, $name:expr) => {
            if let Some(ref r) = $result {
                if !r.errors.is_empty() {
                    parse_error_sheets.push(sheet_result_from_errors($name, &r.errors));
                }
            }
        };
    }
    check_opt_errors!(building_parse, "建筑服务");
    check_opt_errors!(real_estate_parse, "不动产经营租赁服务");
    check_opt_errors!(passenger_parse, "旅客运输服务");
    check_opt_errors!(cargo_parse, "货物运输服务");
    check_opt_errors!(air_parse, "航空运输电子客票行程单");
    check_opt_errors!(railway_parse, "铁路电子客票");
    check_opt_errors!(toll_parse, "通行费数电发票");

    if !parse_error_sheets.is_empty() {
        let mut sheets = vec![SheetImportResult {
            sheet_name: "发票基础信息".to_string(),
            recognized: true,
            inserted: 0,
            error_count: 0,
            errors: vec![],
        }];
        sheets.extend(parse_error_sheets);
        return Ok(ImportSummary {
            batch_no: 0,
            sheets,
            committed: false,
            rollback_reason: Some("明细解析阶段发现错误，已回滚".to_string()),
            seller_count: 0,
            header_inserted: 0,
            detail_inserted: 0,
        });
    }
    eprintln!("[TIMING] parse_detail_sheet: {:?}", t_read3.elapsed());

    let t_read4 = std::time::Instant::now();
    // ── Step 5: 获取连接，应用 PRAGMA，开启事务 ──────────────────────────
    let mut conn_guard = db_conn.lock().unwrap();
    let tx = conn_guard.transaction()?;

    let batch_no = chrono::Utc::now().timestamp() as i32;

    // ── Step 6: 插入发票头（串行，必须先于明细）────────────────────────────
    let mut seller_cache: HashMap<String, i64> = HashMap::new();
    let header_result = insert_headers(&tx, header_parse.rows, batch_no, &mut seller_cache)?;

    if !header_result.errors.is_empty() {
        tx.rollback()?;
        return Ok(ImportSummary {
            batch_no,
            sheets: vec![sheet_result_from_errors(
                "发票基础信息",
                &header_result.errors,
            )],
            committed: false,
            rollback_reason: Some(format!(
                "发票头插入失败：{} 个错误",
                header_result.errors.len()
            )),
            seller_count: usize_to_u32(seller_cache.len()),
            header_inserted: 0,
            detail_inserted: 0,
        });
    }

    let header_id_map = header_result.unq_map;
    let header_inserted = header_result.inserted;

    eprintln!("[TIMING] insert header: {:?}", t_read4.elapsed());
    let t_read5 = std::time::Instant::now();
    // ── Step 7: 串行插入所有明细表 ──────────────────────────────────────
    let mut lnc_summary: HashMap<i64, u32> = HashMap::new();
    let mut lnc_railway: HashMap<i64, u32> = HashMap::new();
    let mut lnc_passenger: HashMap<i64, u32> = HashMap::new();
    let mut lnc_toll: HashMap<i64, u32> = HashMap::new();
    let mut lnc_air: HashMap<i64, u32> = HashMap::new();
    let mut lnc_building: HashMap<i64, u32> = HashMap::new();
    let mut lnc_cargo: HashMap<i64, u32> = HashMap::new();
    let mut lnc_real_estate: HashMap<i64, u32> = HashMap::new();

    macro_rules! do_insert {
        ($insert_fn:expr, $rows:expr, $lnc:expr, $sheet_name:expr) => {{
            match $insert_fn(&tx, $rows, &header_id_map, &mut $lnc) {
                Ok(r) => (
                    r.inserted,
                    make_sheet_result($sheet_name, true, r.inserted, r.errors),
                ),
                Err(e) => (0, make_sheet_result_err($sheet_name, true, e.to_string())),
            }
        }};
        ($insert_fn:expr, $parse_result:expr, $lnc:expr, $sheet_name:expr, optional) => {{
            match $parse_result {
                Some(r) => match $insert_fn(&tx, r.rows, &header_id_map, &mut $lnc) {
                    Ok(ir) => (
                        ir.inserted,
                        make_sheet_result($sheet_name, true, ir.inserted, ir.errors),
                    ),
                    Err(e) => (0, make_sheet_result_err($sheet_name, true, e.to_string())),
                },
                None => (0, make_sheet_result($sheet_name, false, 0, vec![])),
            }
        }};
    }

    let (cnt_summary, sr_summary) = do_insert!(
        insert_details,
        summary_parse.rows,
        lnc_summary,
        "信息汇总表"
    );
    let (cnt_building, sr_building) = do_insert!(
        insert_building_details,
        building_parse,
        lnc_building,
        "建筑服务",
        optional
    );
    let (cnt_real_estate, sr_real_estate) = do_insert!(
        insert_real_estate_details,
        real_estate_parse,
        lnc_real_estate,
        "不动产经营租赁服务",
        optional
    );
    let (cnt_passenger, sr_passenger) = do_insert!(
        insert_passenger_details,
        passenger_parse,
        lnc_passenger,
        "旅客运输服务",
        optional
    );
    let (cnt_cargo, sr_cargo) = do_insert!(
        insert_cargo_details,
        cargo_parse,
        lnc_cargo,
        "货物运输服务",
        optional
    );
    let (cnt_air, sr_air) = do_insert!(
        insert_air_details,
        air_parse,
        lnc_air,
        "航空运输电子客票行程单",
        optional
    );
    let (cnt_railway, sr_railway) = do_insert!(
        insert_railway_details,
        railway_parse,
        lnc_railway,
        "铁路电子客票",
        optional
    );
    let (cnt_toll, sr_toll) = do_insert!(
        insert_toll_details,
        toll_parse,
        lnc_toll,
        "通行费数电发票",
        optional
    );

    // ── Step 8: 检查插入错误，决定提交或回滚 ────────────────────────────
    let all_sheet_results = vec![
        sr_summary,
        sr_building,
        sr_real_estate,
        sr_passenger,
        sr_cargo,
        sr_air,
        sr_railway,
        sr_toll,
    ];

    let has_insert_errors = all_sheet_results.iter().any(|r| r.error_count > 0);

    if has_insert_errors {
        tx.rollback()?;
        let mut sheets = vec![SheetImportResult {
            sheet_name: "发票基础信息".to_string(),
            recognized: true,
            inserted: usize_to_u32(header_inserted),
            error_count: 0,
            errors: vec![],
        }];
        sheets.extend(all_sheet_results);
        return Ok(ImportSummary {
            batch_no,
            sheets,
            committed: false,
            rollback_reason: Some("明细插入阶段发现错误，已回滚".to_string()),
            seller_count: usize_to_u32(seller_cache.len()),
            header_inserted: usize_to_u32(header_inserted),
            detail_inserted: 0,
        });
    }
    eprintln!("[TIMING] insert details: {:?}", t_read5.elapsed());
    // ── Step 9: 提交 ────────────────────────────────────────────────────
    let t_read6 = std::time::Instant::now();
    tx.commit()?;
    eprintln!("[TIMING] commit: {:?}", t_read6.elapsed());

    let detail_inserted = cnt_summary
        + cnt_building
        + cnt_real_estate
        + cnt_passenger
        + cnt_cargo
        + cnt_air
        + cnt_railway
        + cnt_toll;

    let mut sheets = vec![SheetImportResult {
        sheet_name: "发票基础信息".to_string(),
        recognized: true,
        inserted: usize_to_u32(header_inserted),
        error_count: 0,
        errors: vec![],
    }];
    sheets.extend(all_sheet_results);

    Ok(ImportSummary {
        batch_no,
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
            .map(|e| crate::models::dto::CellError {
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
            .map(|e| crate::models::dto::CellError {
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
        errors: vec![crate::models::dto::CellError {
            row: 0,
            col: "unknown".to_string(),
            message,
        }],
    }
}
