// src-tauri/src/db/operations.rs

//
// 设计原则：
//   - upsert_seller: INSERT ... ON CONFLICT DO UPDATE ... RETURNING id
//     一条 SQL 永远返回 id，无需额外 SELECT
//   - insert_headers: INSERT ... RETURNING id 获取 header_id
//     题目保证同批次不重复，冲突则记录错误跳过
//   - 明细表直接 INSERT，不存在冲突场景
//   - batch_size = 1000

use std::collections::HashMap;

use rusqlite::{params, Transaction};

use crate::error::{AppError, RowParseError};
use crate::models::db::{
    ParsedAir, ParsedBuilding, ParsedCargo, ParsedHeader, ParsedInvoiceDetail, ParsedPassenger,
    ParsedRailway, ParsedRealEstate, ParsedToll,
};

// ─────────────────────────────────────────────────────────────
// 返回类型
// ─────────────────────────────────────────────────────────────

/// 明细表插入结果（信息汇总表 + 7 个特种行业表共用）
pub struct InsertResult {
    pub inserted: usize,
    pub errors: Vec<RowParseError>,
}

impl InsertResult {
    fn new() -> Self {
        Self {
            inserted: 0,
            errors: vec![],
        }
    }
}

/// 发票头插入结果（独立结构体，额外携带 unq_map 供明细关联）
pub struct InsertHeadersResult {
    pub inserted: usize,
    pub errors: Vec<RowParseError>,
    /// unq_key → header_id，供所有明细表关联使用
    pub unq_map: HashMap<String, i64>,
    /// 本次入库的 seller 数量（包含已存在的）
    pub seller_count: usize,
}

impl InsertHeadersResult {
    fn new() -> Self {
        Self {
            inserted: 0,
            errors: vec![],
            unq_map: HashMap::new(),
            seller_count: 0,
        }
    }
}

// ─────────────────────────────────────────────────────────────
// 1. 插入 sellers + invoice_headers
// ─────────────────────────────────────────────────────────────

/// 批量插入 sellers 和 invoice_headers。
///
/// 流程：
///   1. 对每行 ParsedHeader，upsert sellers（按 seller_tax_no 去重）
///      - 内存 cache 命中 → 0 次 SQL
///      - INSERT ... ON CONFLICT DO UPDATE SET name = excluded.name RETURNING id
///        无论新行还是已存在，一条 SQL 直接返回 id
///   2. 插入 invoice_headers，INSERT ... RETURNING id 获取 header_id
///      - 题目保证同批次不重复，若 inv_unq_key 冲突则记录错误跳过
///   3. 返回 InsertHeadersResult，包含 unq_map 供后续明细关联
pub fn insert_headers(
    tx: &Transaction,
    rows: Vec<ParsedHeader>,
    batch_no: i32,
    seller_cache: &mut HashMap<String, i64>,
) -> Result<InsertHeadersResult, AppError> {
    let mut result = InsertHeadersResult::new();

    // ✅ prepare_cached 在循环外，只 prepare 一次
    let mut stmt = tx.prepare_cached(
        "INSERT INTO invoice_headers
    (code, num, e_num, seller_id, issue_date,
     amount, tax, total,
     source, inv_type, state, is_pos, risk,
     issuer, remark, rev_org_key, red_notice_no, batch_no)
 VALUES
    (?1, ?2, ?3, ?4, ?5,
     ?6, ?7, ?8,
     ?9, ?10, ?11, ?12, ?13,
     ?14, ?15, ?16, ?17, ?18)
 RETURNING id",
    )?;

    for row in &rows {
        let seller_id = upsert_seller(tx, &row.seller_tax_no, &row.seller_name, seller_cache)?;
        let d = &row.data;

        // CachedStatement 支持 query_row
        let insert_result = stmt.query_row(
            params![
                d.code,
                d.num,
                d.e_num,
                seller_id,
                d.issue_date,
                d.amount,
                d.tax,
                d.total,
                d.source,
                d.inv_type,
                d.state,
                d.is_pos,
                d.risk,
                d.issuer,
                d.remark,
                d.rev_org_key,
                d.red_notice_no,
                batch_no,
            ],
            |r| r.get::<_, i64>(0),
        );

        match insert_result {
            Ok(header_id) => {
                result.unq_map.insert(row.unq_key.clone(), header_id);
                result.inserted += 1;
            }
            Err(e) => {
                result.errors.push(RowParseError {
                    sheet: "发票基础信息".to_string(),
                    row: row.excel_row as u32,
                    col: "发票代码/发票号码/数电发票号码".to_string(),
                    message: format!("插入发票头失败: {}", e),
                });
            }
        }
    }

    result.seller_count = seller_cache.len();
    Ok(result)
}

/// Upsert seller：
///   - 内存 cache 命中 → 直接返回，0 次 SQL
///   - INSERT ... ON CONFLICT(tax_no) DO UPDATE SET name = excluded.name RETURNING id
///     无论新行还是已存在，一条 SQL 直接返回 id，并同步最新名称
fn upsert_seller(
    tx: &Transaction,
    tax_no: &str,
    name: &str,
    cache: &mut HashMap<String, i64>,
) -> Result<i64, AppError> {
    // 1. 内存 cache 命中
    if let Some(&id) = cache.get(tax_no) {
        return Ok(id);
    }

    // 2. 一条 SQL 搭定：新行插入 or 已存在更新 name，均直接 RETURNING id
    let id: i64 = tx.query_row(
        "INSERT INTO sellers (tax_no, name) VALUES (?1, ?2)
         ON CONFLICT(tax_no) DO UPDATE SET name = excluded.name
         RETURNING id",
        params![tax_no, name],
        |r| r.get::<_, i64>(0),
    )?;

    cache.insert(tax_no.to_string(), id);
    Ok(id)
}

// ─────────────────────────────────────────────────────────────
// 2. 插入 invoice_details（信息汇总表）
// ─────────────────────────────────────────────────────────────

/// 批量插入信息汇总表明细。
///
/// ParsedDetail 结构：
///   pub struct ParsedDetail {
///       pub unq_key:   String,   // 对应 header 的 inv_unq_key
///       pub excel_row: usize,    // Excel 行号（用于错误定位）
///       pub data:      InvoiceDetail,
///   }
///
/// InvoiceDetail 字段（对应 invoice_details 表）：
///   header_id  ← 运行时注入（通过 header_id_map 查找）
///   line_no    ← 运行时注入（同一 header_id 下自增）
///   tax_code, biz_type, goods_name, spec, unit,
///   qty, unit_price, amount, tax_rate, tax, total,
///   issuer, remark
pub fn insert_details(
    tx: &Transaction,
    rows: Vec<ParsedInvoiceDetail>,
    header_id_map: &HashMap<String, i64>,
    line_no_counter: &mut HashMap<i64, u32>,
) -> Result<InsertResult, AppError> {
    let mut result = InsertResult::new();
    // line_no 在同一 header_id 下从 1 开始自增

    let mut stmt = tx.prepare_cached(
        "INSERT INTO invoice_details
                (header_id, line_no, tax_code, biz_type, goods_category,goods_name,
                 spec_model, unit, quantity, unit_price, amount, tax_rate, tax,
                 total)
             VALUES
                (?1,  ?2,  ?3,  ?4,  ?5, ?6, 
                ?7,  ?8,  ?9,  ?10, ?11, ?12,?13,
                  ?14)",
    )?;

    for row in rows {
        let header_id = match header_id_map.get(&row.unq_key) {
            Some(&id) => id,
            None => {
                result.errors.push(RowParseError {
                    sheet: "信息汇总表".to_string(),
                    row: row.excel_row as u32,
                    col: "数电发票号码/发票代码+号码".to_string(),
                    message: format!("找不到对应的发票头: {}", row.unq_key),
                });
                continue;
            }
        };

        let line_no = {
            let c = line_no_counter.entry(header_id).or_insert(0);
            *c += 1;
            *c
        };

        let d = &row.data;
        stmt.execute(params![
            header_id,  // ?1
            line_no,    // ?2
            d.tax_code, // ?3  TEXT nullable
            d.biz_type,
            d.goods_category, // ?4  Option<u8> (特定业务类型枚举)
            d.goods_name,     // ?5  TEXT nullable
            d.spec_model,     // ?6  TEXT nullable
            d.unit,           // ?7  TEXT nullable
            d.quantity,       // ?8  REAL nullable
            d.unit_price,     // ?9  REAL nullable
            d.amount,         // ?10 INTEGER (分)
            d.tax_rate,       // ?11 TEXT nullable (如 "13%")
            d.tax,            // ?12 INTEGER (分)
            d.total,          // ?13 INTEGER (分)
        ])?;

        result.inserted += 1;
    }

    Ok(result)
}

// ─────────────────────────────────────────────────────────────
// 3. 插入特种行业明细（7 张表）
// ─────────────────────────────────────────────────────────────

// ── 3a. 铁路电子客票 ──────────────────────────────────────────

/// ParsedRailway.data = RailwayDetail：
///   passenger_name, id_no, depart, arrive, train_no,
///   travel_date, travel_time
///
/// 表字段：
///   header_id  ← 运行时注入
///   line_no    ← 运行时注入
pub fn insert_railway_details(
    tx: &Transaction,
    rows: Vec<ParsedRailway>,
    header_id_map: &HashMap<String, i64>,
    line_no_counter: &mut HashMap<i64, u32>,
) -> Result<InsertResult, AppError> {
    let mut result = InsertResult::new();
    // let mut line_no_counter: HashMap<i64, u32> = HashMap::new();

    let mut stmt = tx.prepare_cached(
        "INSERT INTO railway_details
                (header_id, line_no, passenger_name, id_no,
                 depart, arrival, train_no, travel_date, travel_time)
             VALUES
                (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
    )?;

    for row in rows {
        let header_id = match header_id_map.get(&row.unq_key) {
            Some(&id) => id,
            None => {
                result.errors.push(RowParseError {
                    sheet: "铁路电子客票".to_string(),
                    row: row.excel_row as u32,
                    col: "数电发票号码".to_string(),
                    message: format!("找不到对应的发票头: {}", row.unq_key),
                });
                continue;
            }
        };

        let line_no = {
            let c = line_no_counter.entry(header_id).or_insert(0);
            *c += 1;
            *c
        };

        let d = &row.data;
        stmt.execute(params![
            header_id,        // ?1
            line_no,          // ?2
            d.passenger_name, // ?3  TEXT nullable
            d.id_no,          // ?4  TEXT nullable
            d.depart,         // ?5  TEXT nullable
            d.arrival,        // ?6  TEXT nullable
            d.train_no,       // ?7  TEXT nullable
            d.travel_date,    // ?8  TEXT nullable
            d.travel_time,    // ?9  TEXT nullable
        ])?;

        result.inserted += 1;
    }

    Ok(result)
}

// ── 3b. 旅客运输服务 ──────────────────────────────────────────

/// ParsedPassenger.data = PassengerDetail：
///   passenger_name, id_no, travel_date, depart, arrive,
///   seat_class, transport_type
///
/// 表字段：
///   header_id  ← 运行时注入
///   line_no    ← 运行时注入
pub fn insert_passenger_details(
    tx: &Transaction,
    rows: Vec<ParsedPassenger>,
    header_id_map: &HashMap<String, i64>,
    line_no_counter: &mut HashMap<i64, u32>,
) -> Result<InsertResult, AppError> {
    let mut result = InsertResult::new();
    // let mut line_no_counter: HashMap<i64, u32> = HashMap::new();

    let mut stmt = tx.prepare_cached(
        "INSERT INTO passenger_details
                (header_id, line_no, passenger_name, id_no,
                 travel_date, depart, arrival, seat_class, transport_type)
             VALUES
                (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
    )?;

    for row in rows {
        let header_id = match header_id_map.get(&row.unq_key) {
            Some(&id) => id,
            None => {
                result.errors.push(RowParseError {
                    sheet: "旅客运输服务".to_string(),
                    row: row.excel_row as u32,
                    col: "数电发票号码".to_string(),
                    message: format!("找不到对应的发票头: {}", row.unq_key),
                });
                continue;
            }
        };

        let line_no = {
            let c = line_no_counter.entry(header_id).or_insert(0);
            *c += 1;
            *c
        };

        let d = &row.data;
        stmt.execute(params![
            header_id,        // ?1
            line_no,          // ?2
            d.traveler_name,  // ?3  TEXT nullable
            d.id_no,          // ?4  TEXT nullable
            d.travel_date,    // ?5  TEXT nullable
            d.depart,         // ?6  TEXT nullable
            d.arrival,        // ?7  TEXT nullable
            d.seat_class,     // ?8  TEXT nullable
            d.transport_type, // ?9  TEXT nullable
        ])?;

        result.inserted += 1;
    }

    Ok(result)
}

// ── 3c. 通行费数电发票 ────────────────────────────────────────

/// ParsedToll.data = TollDetail：
///   plate_no, vehicle_type, toll_date_from, toll_date_to
///
/// 表字段：
///   header_id  ← 运行时注入
///   line_no    ← 运行时注入
pub fn insert_toll_details(
    tx: &Transaction,
    rows: Vec<ParsedToll>,
    header_id_map: &HashMap<String, i64>,
    line_no_counter: &mut HashMap<i64, u32>,
) -> Result<InsertResult, AppError> {
    let mut result = InsertResult::new();
    // let mut line_no_counter: HashMap<i64, u32> = HashMap::new();

    let mut stmt = tx.prepare_cached(
        "INSERT INTO toll_details
                (header_id, line_no, plate_no, vehicle_type,
                 toll_date_from, toll_date_to)
             VALUES
                (?1, ?2, ?3, ?4, ?5, ?6)",
    )?;

    for row in rows {
        let header_id = match header_id_map.get(&row.unq_key) {
            Some(&id) => id,
            None => {
                result.errors.push(RowParseError {
                    sheet: "通行费数电发票".to_string(),
                    row: row.excel_row as u32,
                    col: "数电发票号码".to_string(),
                    message: format!("找不到对应的发票头: {}", row.unq_key),
                });
                continue;
            }
        };

        let line_no = {
            let c = line_no_counter.entry(header_id).or_insert(0);
            *c += 1;
            *c
        };

        let d = &row.data;
        stmt.execute(params![
            header_id,         // ?1
            line_no,           // ?2
            d.plate_no,        // ?3  TEXT nullable
            d.vehicle_type,    // ?4  TEXT nullable
            d.pass_start_date, // ?5  TEXT nullable
            d.pass_end_date,   // ?6  TEXT nullable
        ])?;

        result.inserted += 1;
    }

    Ok(result)
}

// ── 3d. 航空运输电子客票行程单 ────────────────────────────────

/// ParsedAir.data = AirDetail：
///   passenger_name, id_no, depart, arrive,
///   carrier, flight_no, flight_date, flight_time
///
/// 表字段：
///   header_id  ← 运行时注入
///   line_no    ← 运行时注入
pub fn insert_air_details(
    tx: &Transaction,
    rows: Vec<ParsedAir>,
    header_id_map: &HashMap<String, i64>,
    line_no_counter: &mut HashMap<i64, u32>,
) -> Result<InsertResult, AppError> {
    let mut result = InsertResult::new();
    // let mut line_no_counter: HashMap<i64, u32> = HashMap::new();

    let mut stmt = tx.prepare_cached(
        "INSERT INTO air_details
                (header_id, line_no, passenger_name, id_no,
                 depart, arrival, carrier, flight_no,
                 flight_date, flight_time)
             VALUES
                (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
    )?;

    for row in rows {
        let header_id = match header_id_map.get(&row.unq_key) {
            Some(&id) => id,
            None => {
                result.errors.push(RowParseError {
                    sheet: "航空运输电子客票行程单".to_string(),
                    row: row.excel_row as u32,
                    col: "数电发票号码".to_string(),
                    message: format!("找不到对应的发票头: {}", row.unq_key),
                });
                continue;
            }
        };

        let line_no = {
            let c = line_no_counter.entry(header_id).or_insert(0);
            *c += 1;
            *c
        };

        let d = &row.data;
        stmt.execute(params![
            header_id,        // ?1
            line_no,          // ?2
            d.passenger_name, // ?3  TEXT nullable
            d.id_no,          // ?4  TEXT nullable
            d.depart,         // ?5  TEXT nullable
            d.arrival,        // ?6  TEXT nullable
            d.carrier,        // ?7  TEXT nullable
            d.flight_no,      // ?8  TEXT nullable
            d.flight_date,    // ?9  TEXT nullable
            d.flight_time,    // ?10 TEXT nullable
        ])?;

        result.inserted += 1;
    }

    Ok(result)
}

// ── 3e. 货物运输服务 ──────────────────────────────────────────

/// ParsedCargo.data = CargoDetail：
///   transport_type, vehicle_no, depart, arrive, goods_name
///
/// 表字段：
///   header_id  ← 运行时注入
///   line_no    ← 运行时注入
pub fn insert_cargo_details(
    tx: &Transaction,
    rows: Vec<ParsedCargo>,
    header_id_map: &HashMap<String, i64>,
    line_no_counter: &mut HashMap<i64, u32>,
) -> Result<InsertResult, AppError> {
    let mut result = InsertResult::new();
    // let mut line_no_counter: HashMap<i64, u32> = HashMap::new();

    let mut stmt = tx.prepare_cached(
        "INSERT INTO cargo_details
                (header_id, line_no, transport_type, vehicle_no,
                 depart, arrival, cargo_name)
             VALUES
                (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
    )?;

    for row in rows {
        let header_id = match header_id_map.get(&row.unq_key) {
            Some(&id) => id,
            None => {
                result.errors.push(RowParseError {
                    sheet: "货物运输服务".to_string(),
                    row: row.excel_row as u32,
                    col: "数电发票号码".to_string(),
                    message: format!("找不到对应的发票头: {}", row.unq_key),
                });
                continue;
            }
        };

        let line_no = {
            let c = line_no_counter.entry(header_id).or_insert(0);
            *c += 1;
            *c
        };

        let d = &row.data;
        stmt.execute(params![
            header_id,          // ?1
            line_no,            // ?2
            d.vehicle_kind,     // ?3  TEXT nullable
            d.vehicle_plate_no, // ?4  TEXT nullable
            d.depart,           // ?5  TEXT nullable
            d.arrival,          // ?6  TEXT nullable
            d.cargo_name,       // ?7  TEXT nullable
        ])?;

        result.inserted += 1;
    }

    Ok(result)
}

// ── 3f. 不动产经营租赁服务 ────────────────────────────────────

/// ParsedRealEstate.data = RealEstateDetail：
///   cert_no, plate_no
///
/// 表字段：
///   header_id  ← 运行时注入
///   line_no    ← 运行时注入
pub fn insert_real_estate_details(
    tx: &Transaction,
    rows: Vec<ParsedRealEstate>,
    header_id_map: &HashMap<String, i64>,
    line_no_counter: &mut HashMap<i64, u32>,
) -> Result<InsertResult, AppError> {
    let mut result = InsertResult::new();
    // let mut line_no_counter: HashMap<i64, u32> = HashMap::new();

    let mut stmt = tx.prepare_cached(
        "INSERT INTO real_estate_details
                (header_id, line_no, cert_no, plate_no)
             VALUES
                (?1, ?2, ?3, ?4)",
    )?;

    for row in rows {
        let header_id = match header_id_map.get(&row.unq_key) {
            Some(&id) => id,
            None => {
                result.errors.push(RowParseError {
                    sheet: "不动产经营租赁服务".to_string(),
                    row: row.excel_row as u32,
                    col: "数电发票号码".to_string(),
                    message: format!("找不到对应的发票头: {}", row.unq_key),
                });
                continue;
            }
        };

        let line_no = {
            let c = line_no_counter.entry(header_id).or_insert(0);
            *c += 1;
            *c
        };

        let d = &row.data;
        stmt.execute(params![
            header_id,  // ?1
            line_no,    // ?2
            d.cert_no,  // ?3  TEXT nullable (产权证书/不动产权证号)
            d.plate_no, // ?4  TEXT nullable (车牌号)
        ])?;

        result.inserted += 1;
    }

    Ok(result)
}

// ── 3g. 建筑服务 ──────────────────────────────────────────────

/// ParsedBuilding.data = BuildingDetail：
///   cross_region_no, service_location, project_name
///
/// 表字段：
///   header_id  ← 运行时注入
///   line_no    ← 运行时注入
pub fn insert_building_details(
    tx: &Transaction,
    rows: Vec<ParsedBuilding>,
    header_id_map: &HashMap<String, i64>,
    line_no_counter: &mut HashMap<i64, u32>,
) -> Result<InsertResult, AppError> {
    let mut result = InsertResult::new();
    // let mut line_no_counter: HashMap<i64, u32> = HashMap::new();

    let mut stmt = tx.prepare_cached(
        "INSERT INTO building_details
                (header_id, line_no, cross_region_no,
                 service_location, project_name)
             VALUES
                (?1, ?2, ?3, ?4, ?5)",
    )?;

    for row in rows {
        let header_id = match header_id_map.get(&row.unq_key) {
            Some(&id) => id,
            None => {
                result.errors.push(RowParseError {
                    sheet: "建筑服务".to_string(),
                    row: row.excel_row as u32,
                    col: "数电发票号码".to_string(),
                    message: format!("找不到对应的发票头: {}", row.unq_key),
                });
                continue;
            }
        };

        let line_no = {
            let c = line_no_counter.entry(header_id).or_insert(0);
            *c += 1;
            *c
        };

        let d = &row.data;
        stmt.execute(params![
            header_id,          // ?1
            line_no,            // ?2
            d.cross_region_no,  // ?3  TEXT nullable (跨区域涉税事项报验管理编号)
            d.service_location, // ?4  TEXT nullable (建筑服务发生地)
            d.project_name,     // ?5  TEXT nullable (建筑项目名称)
        ])?;

        result.inserted += 1;
    }

    Ok(result)
}
