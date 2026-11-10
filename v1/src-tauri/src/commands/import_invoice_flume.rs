// src-tauri/src/commands/import_invoice.rs

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use calamine::{open_workbook, Data, Range, Reader, Xlsx};
use rusqlite::Connection;

use crate::db::operations::{
    insert_air_details, insert_building_details, insert_cargo_details, insert_details,
    insert_headers, insert_passenger_details, insert_railway_details, insert_real_estate_details,
    insert_toll_details,
};

use crate::error::{AppError, RowParseError};
use crate::models::db::{
    ParsedAir, ParsedBuilding, ParsedCargo, ParsedInvoiceDetail, ParsedPassenger, ParsedRailway,
    ParsedRealEstate, ParsedToll,
};
use crate::models::dto::{ImportRequest, ImportSummary, SheetImportResult};
use crate::parser::{
    detail::parse_detail_sheet,
    header::parse_header_sheet,
    special::{
        parse_air_sheet, parse_building_sheet, parse_cargo_sheet, parse_passenger_sheet,
        parse_railway_sheet, parse_real_estate_sheet, parse_toll_sheet,
    },
};
use crate::utils::usize_to_u32;
use crate::DbState;

// ─────────────────────────────────────────────────────────────
// Tauri Command
// ─────────────────────────────────────────────────────────────

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

// ─────────────────────────────────────────────────────────────
// 核心导入逻辑（同步，在 spawn_blocking 中运行）
// ─────────────────────────────────────────────────────────────

fn run_import(file_path: &str, db_conn: Arc<Mutex<Connection>>) -> Result<ImportSummary, AppError> {
    // ── Step 1: 打开 Excel，读取所有 sheet 的 Range ──────────────────────
    let t_read = std::time::Instant::now();

    let mut workbook: Xlsx<_> = open_workbook(file_path)?;

    eprintln!("[TIMING] read_xlsx_sheets: {:?}", t_read.elapsed());
    // 必填 sheet
    let t_read1 = std::time::Instant::now();
    let header_range: Range<Data> = workbook
        .worksheet_range("发票基础信息")
        .map_err(|_| AppError::SheetNotFound("发票基础信息".to_string()))?;
    let summary_range: Range<Data> = workbook
        .worksheet_range("信息汇总表")
        .map_err(|_| AppError::SheetNotFound("信息汇总表".to_string()))?;

    // 可选 sheet（不存在则跳过）
    let building_range = workbook.worksheet_range("建筑服务").ok();
    let real_estate_range = workbook.worksheet_range("不动产经营租赁服务").ok();
    let passenger_range = workbook.worksheet_range("旅客运输服务").ok();
    let cargo_range = workbook.worksheet_range("货物运输服务").ok();
    let air_range = workbook.worksheet_range("航空运输电子客票行程单").ok();
    let railway_range = workbook.worksheet_range("铁路电子客票").ok();
    let toll_range = workbook.worksheet_range("通行费数电发票").ok();

    eprintln!("[TIMING] read_xlsx_sheets: {:?}", t_read1.elapsed());
    // ── Step 2: 阶段1 - 串行处理发票头 ──────────────────────────────────
    // parse_header_sheet 返回具名结构体 HeaderParseResult { rows, errors }
    let t_parse_header = std::time::Instant::now();
    let header_parse = parse_header_sheet(&header_range);
    eprintln!(
        "[TIMING] parse_header_sheet: {:?}",
        t_parse_header.elapsed()
    );
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

    // 获取数据库连接，开启事务
    let mut conn_guard = db_conn.lock().unwrap();

    let tx = conn_guard.transaction()?;

    let batch_no = chrono::Utc::now().timestamp() as i32;

    // 插入发票头，获得 header_id_map（unique_key → header_id）
    // insert_headers 返回 InsertResult { inserted, unq_map, ... }
    // 注意：InsertResult 没有 errors 字段，插入失败直接通过 ? 传播 AppError
    let mut seller_cache: HashMap<String, i64> = HashMap::new();
    let header_insert = insert_headers(&tx, header_parse.rows, batch_no, &mut seller_cache)?;
    let header_id_map = header_insert.unq_map;
    let header_inserted = header_insert.inserted;

    // ── Step 3: 阶段2 - 并行解析所有明细 sheet ──────────────────────────
    //
    // 每个 parser 返回自己的具名结构体，直接用 .rows / .errors 访问。
    // 不包装任何中间层，不强制统一类型。
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
        // 信息汇总表（必填）→ DetailParseResult { rows: Vec<ParsedInvoiceDetail>, errors }
        || parse_detail_sheet(&summary_range),
        || {
            rayon::join(
                // 建筑服务（可选）→ Option<BuildingParseResult { rows: Vec<ParsedBuilding>, errors }>
                || building_range.as_ref().map(|r| parse_building_sheet(r)),
                || {
                    rayon::join(
                        // 不动产经营租赁服务
                        || {
                            real_estate_range
                                .as_ref()
                                .map(|r| parse_real_estate_sheet(r))
                        },
                        || {
                            rayon::join(
                                // 旅客运输服务
                                || passenger_range.as_ref().map(|r| parse_passenger_sheet(r)),
                                || {
                                    rayon::join(
                                        // 货物运输服务
                                        || cargo_range.as_ref().map(|r| parse_cargo_sheet(r)),
                                        || {
                                            rayon::join(
                                                // 航空运输电子客票行程单
                                                || air_range.as_ref().map(|r| parse_air_sheet(r)),
                                                || {
                                                    rayon::join(
                                                        // 铁路电子客票
                                                        || {
                                                            railway_range
                                                                .as_ref()
                                                                .map(|r| parse_railway_sheet(r))
                                                        },
                                                        // 通行费数电发票
                                                        || {
                                                            toll_range
                                                                .as_ref()
                                                                .map(|r| parse_toll_sheet(r))
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

    // ── Step 4: 检查所有明细 sheet 的解析错误 ───────────────────────────
    //
    // 每个具名结构体都有 .errors: Vec<RowParseError>。
    // 可选 sheet 包装在 Option 中，用 if let Some(ref r) = ... 检查。

    let mut parse_error_sheets: Vec<SheetImportResult> = Vec::new();

    // 必填 sheet
    if !summary_parse.errors.is_empty() {
        parse_error_sheets.push(sheet_result_from_errors(
            "信息汇总表",
            &summary_parse.errors,
        ));
    }
    // 可选 sheet：用宏简化重复模式
    macro_rules! check_optional_errors {
        ($result:expr, $name:expr) => {
            if let Some(ref r) = $result {
                if !r.errors.is_empty() {
                    parse_error_sheets.push(sheet_result_from_errors($name, &r.errors));
                }
            }
        };
    }
    check_optional_errors!(building_parse, "建筑服务");
    check_optional_errors!(real_estate_parse, "不动产经营租赁服务");
    check_optional_errors!(passenger_parse, "旅客运输服务");
    check_optional_errors!(cargo_parse, "货物运输服务");
    check_optional_errors!(air_parse, "航空运输电子客票行程单");
    check_optional_errors!(railway_parse, "铁路电子客票");
    check_optional_errors!(toll_parse, "通行费数电发票");

    if !parse_error_sheets.is_empty() {
        tx.rollback()?;
        let mut sheets = vec![SheetImportResult {
            sheet_name: "发票基础信息".to_string(),
            recognized: true,
            inserted: usize_to_u32(header_inserted),
            error_count: 0,
            errors: vec![],
        }];
        sheets.extend(parse_error_sheets);
        return Ok(ImportSummary {
            batch_no,
            sheets,
            committed: false,
            rollback_reason: Some("明细解析阶段发现错误，已回滚".to_string()),
            seller_count: usize_to_u32(seller_cache.len()),
            header_inserted: usize_to_u32(header_inserted),
            detail_inserted: 0,
        });
    }

    // ── Step 5: 使用 flume channel 进行单线程批量插入 ────────────────────
    //
    // 用枚举包装不同类型的行，避免 Box<dyn Any>。
    // 每种 sheet 保留各自的具体类型，消费端按枚举分发到独立 bucket。
    enum DetailRow {
        Summary(ParsedInvoiceDetail),
        Railway(ParsedRailway),
        Passenger(ParsedPassenger),
        Toll(ParsedToll),
        Air(ParsedAir),
        Building(ParsedBuilding),
        Cargo(ParsedCargo),
        RealEstate(ParsedRealEstate),
    }

    let (tx_ch, rx_ch) = flume::unbounded::<(&'static str, DetailRow)>();

    // 生产者：在 rayon scope 中并发发送
    rayon::scope(|s| {
        // 信息汇总表
        {
            let tx_clone = tx_ch.clone();
            let rows = summary_parse.rows;
            s.spawn(move |_| {
                for row in rows {
                    let _ = tx_clone.send(("信息汇总表", DetailRow::Summary(row)));
                }
            });
        }
        // 建筑服务
        if let Some(r) = building_parse {
            let tx_clone = tx_ch.clone();
            s.spawn(move |_| {
                for row in r.rows {
                    let _ = tx_clone.send(("建筑服务", DetailRow::Building(row)));
                }
            });
        }
        // 不动产经营租赁服务
        if let Some(r) = real_estate_parse {
            let tx_clone = tx_ch.clone();
            s.spawn(move |_| {
                for row in r.rows {
                    let _ = tx_clone.send(("不动产经营租赁服务", DetailRow::RealEstate(row)));
                }
            });
        }
        // 旅客运输服务
        if let Some(r) = passenger_parse {
            let tx_clone = tx_ch.clone();
            s.spawn(move |_| {
                for row in r.rows {
                    let _ = tx_clone.send(("旅客运输服务", DetailRow::Passenger(row)));
                }
            });
        }
        // 货物运输服务
        if let Some(r) = cargo_parse {
            let tx_clone = tx_ch.clone();
            s.spawn(move |_| {
                for row in r.rows {
                    let _ = tx_clone.send(("货物运输服务", DetailRow::Cargo(row)));
                }
            });
        }
        // 航空运输电子客票行程单
        if let Some(r) = air_parse {
            let tx_clone = tx_ch.clone();
            s.spawn(move |_| {
                for row in r.rows {
                    let _ = tx_clone.send(("航空运输电子客票行程单", DetailRow::Air(row)));
                }
            });
        }
        // 铁路电子客票
        if let Some(r) = railway_parse {
            let tx_clone = tx_ch.clone();
            s.spawn(move |_| {
                for row in r.rows {
                    let _ = tx_clone.send(("铁路电子客票", DetailRow::Railway(row)));
                }
            });
        }
        // 通行费数电发票
        if let Some(r) = toll_parse {
            let tx_clone = tx_ch.clone();
            s.spawn(move |_| {
                for row in r.rows {
                    let _ = tx_clone.send(("通行费数电发票", DetailRow::Toll(row)));
                }
            });
        }
    });
    drop(tx_ch); // 关闭发送端，消费者循环结束

    // ── Step 6: 单线程消费者 - 按 sheet 分桶，每桶满 2000 条 flush ────────
    const BATCH_SIZE: usize = 2000;
    let mut line_no_summary: HashMap<i64, u32> = HashMap::new();
    let mut line_no_railway: HashMap<i64, u32> = HashMap::new();
    let mut line_no_passenger: HashMap<i64, u32> = HashMap::new();
    let mut line_no_toll: HashMap<i64, u32> = HashMap::new();
    let mut line_no_air: HashMap<i64, u32> = HashMap::new();
    let mut line_no_building: HashMap<i64, u32> = HashMap::new();
    let mut line_no_cargo: HashMap<i64, u32> = HashMap::new();
    let mut line_no_real_estate: HashMap<i64, u32> = HashMap::new();

    let mut bucket_summary: Vec<ParsedInvoiceDetail> = Vec::with_capacity(BATCH_SIZE);
    let mut bucket_railway: Vec<ParsedRailway> = Vec::with_capacity(BATCH_SIZE);
    let mut bucket_passenger: Vec<ParsedPassenger> = Vec::with_capacity(BATCH_SIZE);
    let mut bucket_toll: Vec<ParsedToll> = Vec::with_capacity(BATCH_SIZE);
    let mut bucket_air: Vec<ParsedAir> = Vec::with_capacity(BATCH_SIZE);
    let mut bucket_building: Vec<ParsedBuilding> = Vec::with_capacity(BATCH_SIZE);
    let mut bucket_cargo: Vec<ParsedCargo> = Vec::with_capacity(BATCH_SIZE);
    let mut bucket_real_estate: Vec<ParsedRealEstate> = Vec::with_capacity(BATCH_SIZE);

    let mut cnt_summary: usize = 0;
    let mut cnt_railway: usize = 0;
    let mut cnt_passenger: usize = 0;
    let mut cnt_toll: usize = 0;
    let mut cnt_air: usize = 0;
    let mut cnt_building: usize = 0;
    let mut cnt_cargo: usize = 0;
    let mut cnt_real_estate: usize = 0;

    let mut insert_errors: Vec<AppError> = Vec::new();

    // 辅助宏：将 bucket flush 到数据库。
    // insert_xxx 返回 InsertResult { inserted, unq_map }，没有 errors 字段；
    // 插入失败通过 Err(AppError) 传播。
    macro_rules! flush {
        ($bucket:expr, $cnt:expr, $insert_fn:expr, $lnc:expr) => {{
            if !$bucket.is_empty() {
                let batch = std::mem::take(&mut $bucket);
                match $insert_fn(batch, $lnc) {
                    Ok(r) => $cnt += r.inserted,
                    Err(e) => insert_errors.push(e),
                }
            }
        }};
    }

    for (_sheet_name, row) in rx_ch {
        match row {
            DetailRow::Summary(r) => {
                bucket_summary.push(r);
                if bucket_summary.len() >= BATCH_SIZE {
                    flush!(
                        bucket_summary,
                        cnt_summary,
                        |batch, lnc| insert_details(&tx, batch, &header_id_map, lnc),
                        &mut line_no_summary
                    );
                }
            }
            DetailRow::Railway(r) => {
                bucket_railway.push(r);
                if bucket_railway.len() >= BATCH_SIZE {
                    flush!(
                        bucket_railway,
                        cnt_railway,
                        |batch, lnc| insert_railway_details(&tx, batch, &header_id_map, lnc),
                        &mut line_no_railway
                    );
                }
            }
            DetailRow::Passenger(r) => {
                bucket_passenger.push(r);
                if bucket_passenger.len() >= BATCH_SIZE {
                    flush!(
                        bucket_passenger,
                        cnt_passenger,
                        |batch, lnc| insert_passenger_details(&tx, batch, &header_id_map, lnc),
                        &mut line_no_passenger
                    );
                }
            }
            DetailRow::Toll(r) => {
                bucket_toll.push(r);
                if bucket_toll.len() >= BATCH_SIZE {
                    flush!(
                        bucket_toll,
                        cnt_toll,
                        |batch, lnc| insert_toll_details(&tx, batch, &header_id_map, lnc),
                        &mut line_no_toll
                    );
                }
            }
            DetailRow::Air(r) => {
                bucket_air.push(r);
                if bucket_air.len() >= BATCH_SIZE {
                    flush!(
                        bucket_air,
                        cnt_air,
                        |batch, lnc| insert_air_details(&tx, batch, &header_id_map, lnc),
                        &mut line_no_air
                    );
                }
            }
            DetailRow::Building(r) => {
                bucket_building.push(r);
                if bucket_building.len() >= BATCH_SIZE {
                    flush!(
                        bucket_building,
                        cnt_building,
                        |batch, lnc| insert_building_details(&tx, batch, &header_id_map, lnc),
                        &mut line_no_building
                    );
                }
            }
            DetailRow::Cargo(r) => {
                bucket_cargo.push(r);
                if bucket_cargo.len() >= BATCH_SIZE {
                    flush!(
                        bucket_cargo,
                        cnt_cargo,
                        |batch, lnc| insert_cargo_details(&tx, batch, &header_id_map, lnc),
                        &mut line_no_cargo
                    );
                }
            }
            DetailRow::RealEstate(r) => {
                bucket_real_estate.push(r);
                if bucket_real_estate.len() >= BATCH_SIZE {
                    flush!(
                        bucket_real_estate,
                        cnt_real_estate,
                        |batch, lnc| insert_real_estate_details(&tx, batch, &header_id_map, lnc),
                        &mut line_no_real_estate
                    );
                }
            }
        }
    }

    // 最后 flush 所有剩余数据
    flush!(
        bucket_summary,
        cnt_summary,
        |b, lnc| insert_details(&tx, b, &header_id_map, lnc),
        &mut line_no_summary
    );
    flush!(
        bucket_railway,
        cnt_railway,
        |b, lnc| insert_railway_details(&tx, b, &header_id_map, lnc),
        &mut line_no_railway
    );
    flush!(
        bucket_passenger,
        cnt_passenger,
        |b, lnc| insert_passenger_details(&tx, b, &header_id_map, lnc),
        &mut line_no_passenger
    );
    flush!(
        bucket_toll,
        cnt_toll,
        |b, lnc| insert_toll_details(&tx, b, &header_id_map, lnc),
        &mut line_no_toll
    );
    flush!(
        bucket_air,
        cnt_air,
        |b, lnc| insert_air_details(&tx, b, &header_id_map, lnc),
        &mut line_no_air
    );
    flush!(
        bucket_building,
        cnt_building,
        |b, lnc| insert_building_details(&tx, b, &header_id_map, lnc),
        &mut line_no_building
    );
    flush!(
        bucket_cargo,
        cnt_cargo,
        |b, lnc| insert_cargo_details(&tx, b, &header_id_map, lnc),
        &mut line_no_cargo
    );
    flush!(
        bucket_real_estate,
        cnt_real_estate,
        |b, lnc| insert_real_estate_details(&tx, b, &header_id_map, lnc),
        &mut line_no_real_estate
    );

    // ── Step 7: 检查插入错误，决定提交或回滚 ────────────────────────────
    if !insert_errors.is_empty() {
        tx.rollback()?;
        return Ok(ImportSummary {
            batch_no,
            sheets: vec![
                SheetImportResult {
                    sheet_name: "发票基础信息".to_string(),
                    recognized: true,
                    inserted: usize_to_u32(header_inserted),
                    error_count: 0,
                    errors: vec![],
                },
                SheetImportResult {
                    sheet_name: "明细数据".to_string(),
                    recognized: true,
                    inserted: 0,
                    error_count: usize_to_u32(insert_errors.len()),
                    errors: insert_errors
                        .iter()
                        .map(|e| crate::models::dto::CellError {
                            row: 0,
                            col: "unknown".to_string(),
                            message: e.to_string(),
                        })
                        .collect(),
                },
            ],
            committed: false,
            rollback_reason: Some(format!("明细插入失败：{} 个错误", insert_errors.len())),
            seller_count: usize_to_u32(seller_cache.len()),
            header_inserted: usize_to_u32(header_inserted),
            detail_inserted: 0,
        });
    }

    // ── Step 8: 提交事务 ────────────────────────────────────────────────
    tx.commit()?;

    let detail_inserted = cnt_summary
        + cnt_railway
        + cnt_passenger
        + cnt_toll
        + cnt_air
        + cnt_building
        + cnt_cargo
        + cnt_real_estate;

    // ── Step 9: 返回成功结果 ────────────────────────────────────────────
    Ok(ImportSummary {
        batch_no,
        sheets: vec![
            SheetImportResult {
                sheet_name: "发票基础信息".to_string(),
                recognized: true,
                inserted: usize_to_u32(header_inserted),
                error_count: 0,
                errors: vec![],
            },
            SheetImportResult {
                sheet_name: "信息汇总表".to_string(),
                recognized: true,
                inserted: usize_to_u32(cnt_summary),
                error_count: 0,
                errors: vec![],
            },
            SheetImportResult {
                sheet_name: "建筑服务".to_string(),
                recognized: building_range.is_some(),
                inserted: usize_to_u32(cnt_building),
                error_count: 0,
                errors: vec![],
            },
            SheetImportResult {
                sheet_name: "不动产经营租赁服务".to_string(),
                recognized: real_estate_range.is_some(),
                inserted: usize_to_u32(cnt_real_estate),
                error_count: 0,
                errors: vec![],
            },
            SheetImportResult {
                sheet_name: "旅客运输服务".to_string(),
                recognized: passenger_range.is_some(),
                inserted: usize_to_u32(cnt_passenger),
                error_count: 0,
                errors: vec![],
            },
            SheetImportResult {
                sheet_name: "货物运输服务".to_string(),
                recognized: cargo_range.is_some(),
                inserted: usize_to_u32(cnt_cargo),
                error_count: 0,
                errors: vec![],
            },
            SheetImportResult {
                sheet_name: "航空运输电子客票行程单".to_string(),
                recognized: air_range.is_some(),
                inserted: usize_to_u32(cnt_air),
                error_count: 0,
                errors: vec![],
            },
            SheetImportResult {
                sheet_name: "铁路电子客票".to_string(),
                recognized: railway_range.is_some(),
                inserted: usize_to_u32(cnt_railway),
                error_count: 0,
                errors: vec![],
            },
            SheetImportResult {
                sheet_name: "通行费数电发票".to_string(),
                recognized: toll_range.is_some(),
                inserted: usize_to_u32(cnt_toll),
                error_count: 0,
                errors: vec![],
            },
        ],
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

/// 从错误列表构建一个已回滚的 SheetImportResult
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
