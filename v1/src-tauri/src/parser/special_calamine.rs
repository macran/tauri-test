// src-tauri/src/parser/special.rs
//
// 解析 7 种特种业务类型发票明细 sheet。
//
// 风格与 header.rs / detail.rs 完全一致：
//   parse_xxx_sheet  →  行过滤 + 调用 parse_one_row
//   parse_one_row    →  Raw层转换 → unq_key → 组装 Parsed，用 AppError + ? 传播
//   RowParseError::from_app_error  →  统一转换

use calamine::{Data, Range};

use crate::error::{AppError, RowParseError};
use crate::models::db::{
    AirDetail, BuildingDetail, CargoDetail, ParsedAir, ParsedBuilding, ParsedCargo,
    ParsedPassenger, ParsedRailway, ParsedRealEstate, ParsedToll, PassengerDetail, RailwayDetail,
    RealEstateDetail, TollDetail,
};
use crate::models::raw_row::{
    AirRaw, BuildingRaw, CargoRaw, PassengerRaw, RailwayRaw, RealEstateRaw, TollRaw,
};
use crate::utils::{build_unq_key, usize_to_u32};

// ─────────────────────────────────────────────────────────────
// 公共返回类型（与 HeaderParseResult / DetailParseResult 对称）
// ─────────────────────────────────────────────────────────────

pub struct SpecialParseResult<T> {
    pub rows: Vec<T>,
    pub errors: Vec<RowParseError>,
}

// ─────────────────────────────────────────────────────────────
// COL_NAMES：每个 sheet 的完整列名表
// 用于将 col_idx 映射为可读列名（错误信息用）
// ─────────────────────────────────────────────────────────────

/// 所有特种 sheet 共用的前缀列名（0~15）
// const COMMON_COL_NAMES: &[&str] = &[
//     "序号",         // 0
//     "发票代码",     // 1
//     "发票号码",     // 2
//     "数电发票号码", // 3
//     "销方识别号",   // 4
//     "销方名称",     // 5
//     "购方识别号",   // 6
//     "购买方名称",   // 7
//     "开票日期",     // 8
//     "发票来源",     // 9
//     "发票票种",     // 10
//     "发票状态",     // 11
//     "是否正数发票", // 12
//     "发票风险等级", // 13
//     "开票人",       // 14
//     "备注",         // 15
// ];

// const RAILWAY_COL_NAMES: &[&str] = &[
//     "旅客姓名",     // 16
//     "有效身份证号", // 17
//     "出发地",       // 18
//     "到达地",       // 19
//     "出行车次",     // 20
//     "日期",         // 21
//     "时间",         // 22
// ];

// const PASSENGER_COL_NAMES: &[&str] = &[
//     "出行人",       // 16
//     "有效身份证号", // 17
//     "出行日期",     // 18
//     "出发地",       // 19
//     "到达地",       // 20
//     "等级",         // 21
//     "交通工具类型", // 22
// ];

// const TOLL_COL_NAMES: &[&str] = &[
//     "车牌号码",   // 16
//     "车辆类型",   // 17
//     "通行日期起", // 18
//     "通行日期止", // 19
// ];

// const BUILDING_COL_NAMES: &[&str] = &[
//     "跨区域涉税事项报验管理编号", // 16
//     "建筑服务发生地",             // 17
//     "建筑项目名称",               // 18
// ];

// const CARGO_COL_NAMES: &[&str] = &[
//     "运输工具种类", // 16
//     "运输工具牌号", // 17
//     "起运地",       // 18
//     "到达地",       // 19
//     "运输货物名称", // 20
// ];

// const REAL_ESTATE_COL_NAMES: &[&str] = &[
//     "产权证书/不动产权证号", // 16
//     "车牌号",                // 17
// ];

// const AIR_COL_NAMES: &[&str] = &[
//     "旅客姓名",     // 16
//     "有效身份证号", // 17
//     "出发地",       // 18
//     "到达地",       // 19
//     "承运人",       // 20
//     "航班号",       // 21
//     "日期",         // 22
//     "时间",         // 23
// ];

// ─────────────────────────────────────────────────────────────
// 内部工具：col_idx → 列名
// ─────────────────────────────────────────────────────────────

// fn col_name<'a>(col_idx: usize, ext_names: &[&'a str]) -> &'a str {
//     if col_idx < COMMON_COL_NAMES.len() {
//         COMMON_COL_NAMES[col_idx]
//     } else {
//         let ext_idx = col_idx - COMMON_COL_NAMES.len();
//         ext_names.get(ext_idx).copied().unwrap_or("未知列")
//     }
// }

// ─────────────────────────────────────────────────────────────
// 内部工具：build_unq_key 封装（与 header.rs / detail.rs 完全一致）
// ─────────────────────────────────────────────────────────────

fn make_unq_key(
    code: Option<&str>,
    num: Option<&str>,
    e_num: Option<&str>,
    sheet: &'static str,
    row: u32,
) -> Result<String, AppError> {
    build_unq_key(code, num, e_num).ok_or(AppError::MissingUnqKey { sheet, row })
}

// ─────────────────────────────────────────────────────────────
// 1. 铁路电子客票
// ─────────────────────────────────────────────────────────────

const SHEET_RAILWAY: &str = "铁路电子客票";

pub fn parse_railway_sheet(range: &Range<Data>) -> SpecialParseResult<ParsedRailway> {
    let mut rows = Vec::new();
    let mut errors = Vec::new();

    for (i, row) in range.rows().enumerate().skip(1) {
        let excel_row = i + 1;

        if row.iter().all(|c| matches!(c, Data::Empty)) {
            continue;
        }

        match parse_railway_row(row, excel_row) {
            Ok(parsed) => rows.push(parsed),
            Err(e) => {
                if let Some(re) = RowParseError::from_app_error(&e) {
                    errors.push(re);
                }
            }
        }
    }

    SpecialParseResult { rows, errors }
}

fn parse_railway_row(row: &[Data], row_idx: usize) -> Result<ParsedRailway, AppError> {
    let row_u32 = usize_to_u32(row_idx);

    // ── Step 1: Raw 层转换 ──────────────────────────────────────
    let raw = RailwayRaw::from_row(row);

    // ── Step 2: unq_key ─────────────────────────────────────────
    let unq_key = make_unq_key(
        raw.key.code.as_deref(),
        raw.key.num.as_deref(),
        raw.key.e_num.as_deref(),
        SHEET_RAILWAY,
        row_u32,
    )?;

    // ── Step 3: 日期格式化 ──────────────────────────────────────
    // let travel_date = raw.travel_date.map(|s| parse_date_str(&s).unwrap_or(s));

    // ── Step 4: 组装 ────────────────────────────────────────────
    Ok(ParsedRailway {
        unq_key,
        excel_row: row_idx,
        data: RailwayDetail {
            passenger_name: raw.passenger_name,
            id_no: raw.id_no,
            depart: raw.depart,
            arrival: raw.arrive,
            train_no: raw.train_no,
            travel_date: raw.travel_date,
            travel_time: raw.travel_time,
        },
    })
}

// ─────────────────────────────────────────────────────────────
// 2. 旅客运输服务
// ─────────────────────────────────────────────────────────────

const SHEET_PASSENGER: &str = "旅客运输服务";

pub fn parse_passenger_sheet(range: &Range<Data>) -> SpecialParseResult<ParsedPassenger> {
    let mut rows = Vec::new();
    let mut errors = Vec::new();

    for (i, row) in range.rows().enumerate().skip(1) {
        let excel_row = i + 1;

        if row.iter().all(|c| matches!(c, Data::Empty)) {
            continue;
        }

        match parse_passenger_row(row, excel_row) {
            Ok(parsed) => rows.push(parsed),
            Err(e) => {
                if let Some(re) = RowParseError::from_app_error(&e) {
                    errors.push(re);
                }
            }
        }
    }

    SpecialParseResult { rows, errors }
}

fn parse_passenger_row(row: &[Data], row_idx: usize) -> Result<ParsedPassenger, AppError> {
    let row_u32 = usize_to_u32(row_idx);

    let raw = PassengerRaw::from_row(row);

    let unq_key = make_unq_key(
        raw.key.code.as_deref(),
        raw.key.num.as_deref(),
        raw.key.e_num.as_deref(),
        SHEET_PASSENGER,
        row_u32,
    )?;

    // let travel_date = raw.travel_date.map(|s| parse_date_str(&s).unwrap_or(s));

    // 网约车发票 id_no / depart / arrive 可能为 "-"，过滤掉
    let id_no = raw.id_no.filter(|s| s != "-" && !s.is_empty());
    let depart = raw.depart.filter(|s| s != "-" && !s.is_empty());
    let arrive = raw.arrive.filter(|s| s != "-" && !s.is_empty());
    let seat_class = raw.seat_class.filter(|s| !s.is_empty());

    Ok(ParsedPassenger {
        unq_key,
        excel_row: row_idx,
        data: PassengerDetail {
            traveler_name: raw.traveler,
            id_no: id_no,
            travel_date: raw.travel_date,
            depart: depart,
            arrival: arrive,
            seat_class: seat_class,
            transport_type: raw.transport_type,
        },
    })
}

// ─────────────────────────────────────────────────────────────
// 3. 通行费数电发票
// ─────────────────────────────────────────────────────────────

const SHEET_TOLL: &str = "通行费数电发票";

pub fn parse_toll_sheet(range: &Range<Data>) -> SpecialParseResult<ParsedToll> {
    let mut rows = Vec::new();
    let mut errors = Vec::new();

    for (i, row) in range.rows().enumerate().skip(1) {
        let excel_row = i + 1;

        if row.iter().all(|c| matches!(c, Data::Empty)) {
            continue;
        }

        match parse_toll_row(row, excel_row) {
            Ok(parsed) => rows.push(parsed),
            Err(e) => {
                if let Some(re) = RowParseError::from_app_error(&e) {
                    errors.push(re);
                }
            }
        }
    }

    SpecialParseResult { rows, errors }
}

fn parse_toll_row(row: &[Data], row_idx: usize) -> Result<ParsedToll, AppError> {
    let row_u32 = usize_to_u32(row_idx);

    let raw = TollRaw::from_row(row);

    let unq_key = make_unq_key(
        raw.key.code.as_deref(),
        raw.key.num.as_deref(),
        raw.key.e_num.as_deref(),
        SHEET_TOLL,
        row_u32,
    )?;

    // let toll_date_from = raw.toll_date_from.map(|s| parse_date_str(&s).unwrap_or(s));
    // let toll_date_to = raw.toll_date_to.map(|s| parse_date_str(&s).unwrap_or(s));

    Ok(ParsedToll {
        unq_key,
        excel_row: row_idx,
        data: TollDetail {
            plate_no: raw.plate_no,
            vehicle_type: raw.vehicle_type,
            pass_start_date: raw.toll_date_from,
            pass_end_date: raw.toll_date_to,
        },
    })
}

// ─────────────────────────────────────────────────────────────
// 4. 建筑服务
// ─────────────────────────────────────────────────────────────

const SHEET_BUILDING: &str = "建筑服务";

pub fn parse_building_sheet(range: &Range<Data>) -> SpecialParseResult<ParsedBuilding> {
    let mut rows = Vec::new();
    let mut errors = Vec::new();

    for (i, row) in range.rows().enumerate().skip(1) {
        let excel_row = i + 1;

        if row.iter().all(|c| matches!(c, Data::Empty)) {
            continue;
        }

        match parse_building_row(row, excel_row) {
            Ok(parsed) => rows.push(parsed),
            Err(e) => {
                if let Some(re) = RowParseError::from_app_error(&e) {
                    errors.push(re);
                }
            }
        }
    }

    SpecialParseResult { rows, errors }
}

fn parse_building_row(row: &[Data], row_idx: usize) -> Result<ParsedBuilding, AppError> {
    let row_u32 = usize_to_u32(row_idx);

    let raw = BuildingRaw::from_row(row);

    let unq_key = make_unq_key(
        raw.key.code.as_deref(),
        raw.key.num.as_deref(),
        raw.key.e_num.as_deref(),
        SHEET_BUILDING,
        row_u32,
    )?;

    let cross_region_no = raw.cross_region_no.filter(|s| !s.is_empty());
    let service_location = raw.service_location.filter(|s| !s.is_empty());

    Ok(ParsedBuilding {
        unq_key,
        excel_row: row_idx,
        data: BuildingDetail {
            cross_region_no: cross_region_no,
            service_location: service_location,
            project_name: raw.project_name,
        },
    })
}

// ─────────────────────────────────────────────────────────────
// 5. 货物运输服务
// ─────────────────────────────────────────────────────────────

const SHEET_CARGO: &str = "货物运输服务";

pub fn parse_cargo_sheet(range: &Range<Data>) -> SpecialParseResult<ParsedCargo> {
    let mut rows = Vec::new();
    let mut errors = Vec::new();

    for (i, row) in range.rows().enumerate().skip(1) {
        let excel_row = i + 1;

        if row.iter().all(|c| matches!(c, Data::Empty)) {
            continue;
        }

        match parse_cargo_row(row, excel_row) {
            Ok(parsed) => rows.push(parsed),
            Err(e) => {
                if let Some(re) = RowParseError::from_app_error(&e) {
                    errors.push(re);
                }
            }
        }
    }

    SpecialParseResult { rows, errors }
}

fn parse_cargo_row(row: &[Data], row_idx: usize) -> Result<ParsedCargo, AppError> {
    let row_u32 = usize_to_u32(row_idx);

    let raw = CargoRaw::from_row(row);

    let unq_key = make_unq_key(
        raw.key.code.as_deref(),
        raw.key.num.as_deref(),
        raw.key.e_num.as_deref(),
        SHEET_CARGO,
        row_u32,
    )?;

    let vehicle_plate_no = raw.vehicle_no.filter(|s| !s.is_empty());
    let depart = raw.depart.filter(|s| !s.is_empty());
    let arrival = raw.arrive.filter(|s| !s.is_empty());

    Ok(ParsedCargo {
        unq_key,
        excel_row: row_idx,
        data: CargoDetail {
            vehicle_kind: raw.transport_type,
            vehicle_plate_no,
            depart,
            arrival,
            cargo_name: raw.goods_name,
        },
    })
}

// ─────────────────────────────────────────────────────────────
// 6. 不动产经营租赁服务
// ─────────────────────────────────────────────────────────────

const SHEET_REAL_ESTATE: &str = "不动产经营租赁服务";

pub fn parse_real_estate_sheet(range: &Range<Data>) -> SpecialParseResult<ParsedRealEstate> {
    let mut rows = Vec::new();
    let mut errors = Vec::new();

    for (i, row) in range.rows().enumerate().skip(1) {
        let excel_row = i + 1;

        if row.iter().all(|c| matches!(c, Data::Empty)) {
            continue;
        }

        match parse_real_estate_row(row, excel_row) {
            Ok(parsed) => rows.push(parsed),
            Err(e) => {
                if let Some(re) = RowParseError::from_app_error(&e) {
                    errors.push(re);
                }
            }
        }
    }

    SpecialParseResult { rows, errors }
}

fn parse_real_estate_row(row: &[Data], row_idx: usize) -> Result<ParsedRealEstate, AppError> {
    let row_u32 = usize_to_u32(row_idx);

    // RealEstateRaw::from_row 不会返回 Err（所有字段均可选），
    // 但保持与其他 sheet 一致的调用形式
    let raw = RealEstateRaw::from_row(row);

    let unq_key = make_unq_key(
        raw.key.code.as_deref(),
        raw.key.num.as_deref(),
        raw.key.e_num.as_deref(),
        SHEET_REAL_ESTATE,
        row_u32,
    )?;

    let cert_no = raw.property_cert_no.filter(|s| !s.is_empty());
    let plate_no = raw.plate_no.filter(|s| !s.is_empty());

    Ok(ParsedRealEstate {
        unq_key,
        excel_row: row_idx,
        data: RealEstateDetail { cert_no, plate_no },
    })
}

// ─────────────────────────────────────────────────────────────
// 7. 航空运输电子客票行程单
// ─────────────────────────────────────────────────────────────

const SHEET_AIR: &str = "航空运输电子客票行程单";

pub fn parse_air_sheet(range: &Range<Data>) -> SpecialParseResult<ParsedAir> {
    let mut rows = Vec::new();
    let mut errors = Vec::new();

    for (i, row) in range.rows().enumerate().skip(1) {
        let excel_row = i + 1;

        if row.iter().all(|c| matches!(c, Data::Empty)) {
            continue;
        }

        match parse_air_row(row, excel_row) {
            Ok(parsed) => rows.push(parsed),
            Err(e) => {
                if let Some(re) = RowParseError::from_app_error(&e) {
                    errors.push(re);
                }
            }
        }
    }

    SpecialParseResult { rows, errors }
}

fn parse_air_row(row: &[Data], row_idx: usize) -> Result<ParsedAir, AppError> {
    let row_u32 = usize_to_u32(row_idx);

    let raw = AirRaw::from_row(row);

    let unq_key = make_unq_key(
        raw.key.code.as_deref(),
        raw.key.num.as_deref(),
        raw.key.e_num.as_deref(),
        SHEET_AIR,
        row_u32,
    )?;

    // let flight_date = raw.flight_date.map(|s| parse_date_str(&s).unwrap_or(s));
    // let flight_time = raw.flight_time.map(|s| parse_date_str(&s).unwrap_or(s));
    let id_no = raw.id_no.filter(|s| !s.is_empty());

    Ok(ParsedAir {
        unq_key,
        excel_row: row_idx,
        data: AirDetail {
            passenger_name: raw.passenger_name,
            id_no,
            depart: raw.depart,
            arrival: raw.arrive,
            carrier: raw.carrier,
            flight_no: raw.flight_no,
            flight_date: raw.flight_date,
            flight_time: raw.flight_time,
        },
    })
}
