use crate::error::{AppError, RowParseError};
use crate::excel_reader::{SheetData, SheetRow};
use crate::utils::{build_unq_key, is_empty_row, is_total_row, usize_to_u32};

use super::models::{
    AirDetail, AirRaw, BuildingDetail, BuildingRaw, CargoDetail, CargoRaw, ParsedAir,
    ParsedBuilding, ParsedCargo, ParsedPassenger, ParsedRailway, ParsedRealEstate, ParsedToll,
    PassengerDetail, PassengerRaw, RailwayDetail, RailwayRaw, RealEstateDetail, RealEstateRaw,
    TollDetail, TollRaw,
};

pub struct ParseResult<T> {
    pub rows: Vec<T>,
    pub errors: Vec<RowParseError>,
}

// ─────────────────────────────────────────────────────────────
// 公共辅助
// ─────────────────────────────────────────────────────────────

fn build_unq_key_from_parts(
    sheet: &'static str,
    row_u32: u32,
    code: Option<&str>,
    num: Option<&str>,
    e_num: Option<&str>,
) -> Result<String, AppError> {
    build_unq_key(code, num, e_num).ok_or(AppError::MissingUnqKey {
        sheet,
        row: row_u32,
    })
}

// ─────────────────────────────────────────────────────────────
// 铁路电子客票
// ─────────────────────────────────────────────────────────────

pub fn parse_railway_sheet(sheet: &SheetData, shared: &[String]) -> ParseResult<ParsedRailway> {
    let mut rows = Vec::new();
    let mut errors = Vec::new();

    for (i, row) in sheet.data_rows().iter().enumerate() {
        let excel_row = i + 2;
        if is_empty_row(row) || is_total_row(row, shared) {
            continue;
        }

        match parse_railway_row(row, excel_row, shared) {
            Ok(v) => rows.push(v),
            Err(e) => {
                if let Some(re) = RowParseError::from_app_error(&e) {
                    errors.push(re);
                }
            }
        }
    }

    ParseResult { rows, errors }
}

fn parse_railway_row(
    row: &SheetRow,
    row_idx: usize,
    shared: &[String],
) -> Result<ParsedRailway, AppError> {
    const SHEET: &str = "铁路电子客票";
    let row_u32 = usize_to_u32(row_idx);
    let raw = RailwayRaw::from_row(row, shared);

    let unq_key = build_unq_key_from_parts(
        SHEET,
        row_u32,
        raw.key.code.as_deref(),
        raw.key.num.as_deref(),
        raw.key.e_num.as_deref(),
    )?;

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
// 旅客运输服务
// ─────────────────────────────────────────────────────────────

pub fn parse_passenger_sheet(sheet: &SheetData, shared: &[String]) -> ParseResult<ParsedPassenger> {
    let mut rows = Vec::new();
    let mut errors = Vec::new();

    for (i, row) in sheet.data_rows().iter().enumerate() {
        let excel_row = i + 2;
        if is_empty_row(row) || is_total_row(row, shared) {
            continue;
        }

        match parse_passenger_row(row, excel_row, shared) {
            Ok(v) => rows.push(v),
            Err(e) => {
                if let Some(re) = RowParseError::from_app_error(&e) {
                    errors.push(re);
                }
            }
        }
    }

    ParseResult { rows, errors }
}

fn parse_passenger_row(
    row: &SheetRow,
    row_idx: usize,
    shared: &[String],
) -> Result<ParsedPassenger, AppError> {
    const SHEET: &str = "旅客运输服务";
    let row_u32 = usize_to_u32(row_idx);
    let raw = PassengerRaw::from_row(row, shared);

    let unq_key = build_unq_key_from_parts(
        SHEET,
        row_u32,
        raw.key.code.as_deref(),
        raw.key.num.as_deref(),
        raw.key.e_num.as_deref(),
    )?;

    Ok(ParsedPassenger {
        unq_key,
        excel_row: row_idx,
        data: PassengerDetail {
            traveler_name: raw.traveler,
            id_no: raw.id_no,
            travel_date: raw.travel_date,
            depart: raw.depart,
            arrival: raw.arrive,
            seat_class: raw.seat_class,
            transport_type: raw.transport_type,
        },
    })
}

// ─────────────────────────────────────────────────────────────
// 通行费数电发票
// ─────────────────────────────────────────────────────────────

pub fn parse_toll_sheet(sheet: &SheetData, shared: &[String]) -> ParseResult<ParsedToll> {
    let mut rows = Vec::new();
    let mut errors = Vec::new();

    for (i, row) in sheet.data_rows().iter().enumerate() {
        let excel_row = i + 2;
        if is_empty_row(row) || is_total_row(row, shared) {
            continue;
        }

        match parse_toll_row(row, excel_row, shared) {
            Ok(v) => rows.push(v),
            Err(e) => {
                if let Some(re) = RowParseError::from_app_error(&e) {
                    errors.push(re);
                }
            }
        }
    }

    ParseResult { rows, errors }
}

fn parse_toll_row(
    row: &SheetRow,
    row_idx: usize,
    shared: &[String],
) -> Result<ParsedToll, AppError> {
    const SHEET: &str = "通行费数电发票";
    let row_u32 = usize_to_u32(row_idx);
    let raw = TollRaw::from_row(row, shared);

    let unq_key = build_unq_key_from_parts(
        SHEET,
        row_u32,
        raw.key.code.as_deref(),
        raw.key.num.as_deref(),
        raw.key.e_num.as_deref(),
    )?;

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
// 建筑服务
// ─────────────────────────────────────────────────────────────

pub fn parse_building_sheet(sheet: &SheetData, shared: &[String]) -> ParseResult<ParsedBuilding> {
    let mut rows = Vec::new();
    let mut errors = Vec::new();

    for (i, row) in sheet.data_rows().iter().enumerate() {
        let excel_row = i + 2;
        if is_empty_row(row) || is_total_row(row, shared) {
            continue;
        }

        match parse_building_row(row, excel_row, shared) {
            Ok(v) => rows.push(v),
            Err(e) => {
                if let Some(re) = RowParseError::from_app_error(&e) {
                    errors.push(re);
                }
            }
        }
    }

    ParseResult { rows, errors }
}

fn parse_building_row(
    row: &SheetRow,
    row_idx: usize,
    shared: &[String],
) -> Result<ParsedBuilding, AppError> {
    const SHEET: &str = "建筑服务";
    let row_u32 = usize_to_u32(row_idx);
    let raw = BuildingRaw::from_row(row, shared);

    let unq_key = build_unq_key_from_parts(
        SHEET,
        row_u32,
        raw.key.code.as_deref(),
        raw.key.num.as_deref(),
        raw.key.e_num.as_deref(),
    )?;

    Ok(ParsedBuilding {
        unq_key,
        excel_row: row_idx,
        data: BuildingDetail {
            cross_region_no: raw.cross_region_no,
            service_location: raw.service_location,
            project_name: raw.project_name,
        },
    })
}

// ─────────────────────────────────────────────────────────────
// 货物运输服务
// ─────────────────────────────────────────────────────────────

pub fn parse_cargo_sheet(sheet: &SheetData, shared: &[String]) -> ParseResult<ParsedCargo> {
    let mut rows = Vec::new();
    let mut errors = Vec::new();

    for (i, row) in sheet.data_rows().iter().enumerate() {
        let excel_row = i + 2;
        if is_empty_row(row) || is_total_row(row, shared) {
            continue;
        }

        match parse_cargo_row(row, excel_row, shared) {
            Ok(v) => rows.push(v),
            Err(e) => {
                if let Some(re) = RowParseError::from_app_error(&e) {
                    errors.push(re);
                }
            }
        }
    }

    ParseResult { rows, errors }
}

fn parse_cargo_row(
    row: &SheetRow,
    row_idx: usize,
    shared: &[String],
) -> Result<ParsedCargo, AppError> {
    const SHEET: &str = "货物运输服务";
    let row_u32 = usize_to_u32(row_idx);
    let raw = CargoRaw::from_row(row, shared);

    let unq_key = build_unq_key_from_parts(
        SHEET,
        row_u32,
        raw.key.code.as_deref(),
        raw.key.num.as_deref(),
        raw.key.e_num.as_deref(),
    )?;

    Ok(ParsedCargo {
        unq_key,
        excel_row: row_idx,
        data: CargoDetail {
            vehicle_kind: raw.transport_type,
            vehicle_plate_no: raw.vehicle_no,
            depart: raw.depart,
            arrival: raw.arrive,
            cargo_name: raw.goods_name,
        },
    })
}

// ─────────────────────────────────────────────────────────────
// 不动产经营租赁服务
// ─────────────────────────────────────────────────────────────

pub fn parse_real_estate_sheet(
    sheet: &SheetData,
    shared: &[String],
) -> ParseResult<ParsedRealEstate> {
    let mut rows = Vec::new();
    let mut errors = Vec::new();

    for (i, row) in sheet.data_rows().iter().enumerate() {
        let excel_row = i + 2;
        if is_empty_row(row) || is_total_row(row, shared) {
            continue;
        }

        match parse_real_estate_row(row, excel_row, shared) {
            Ok(v) => rows.push(v),
            Err(e) => {
                if let Some(re) = RowParseError::from_app_error(&e) {
                    errors.push(re);
                }
            }
        }
    }

    ParseResult { rows, errors }
}

fn parse_real_estate_row(
    row: &SheetRow,
    row_idx: usize,
    shared: &[String],
) -> Result<ParsedRealEstate, AppError> {
    const SHEET: &str = "不动产经营租赁服务";
    let row_u32 = usize_to_u32(row_idx);
    let raw = RealEstateRaw::from_row(row, shared);

    let unq_key = build_unq_key_from_parts(
        SHEET,
        row_u32,
        raw.key.code.as_deref(),
        raw.key.num.as_deref(),
        raw.key.e_num.as_deref(),
    )?;

    Ok(ParsedRealEstate {
        unq_key,
        excel_row: row_idx,
        data: RealEstateDetail {
            cert_no: raw.property_cert_no,
            plate_no: raw.plate_no,
        },
    })
}

// ─────────────────────────────────────────────────────────────
// 航空运输电子客票行程单
// ─────────────────────────────────────────────────────────────

pub fn parse_air_sheet(sheet: &SheetData, shared: &[String]) -> ParseResult<ParsedAir> {
    let mut rows = Vec::new();
    let mut errors = Vec::new();

    for (i, row) in sheet.data_rows().iter().enumerate() {
        let excel_row = i + 2;
        if is_empty_row(row) || is_total_row(row, shared) {
            continue;
        }

        match parse_air_row(row, excel_row, shared) {
            Ok(v) => rows.push(v),
            Err(e) => {
                if let Some(re) = RowParseError::from_app_error(&e) {
                    errors.push(re);
                }
            }
        }
    }

    ParseResult { rows, errors }
}

fn parse_air_row(row: &SheetRow, row_idx: usize, shared: &[String]) -> Result<ParsedAir, AppError> {
    const SHEET: &str = "航空运输电子客票行程单";
    let row_u32 = usize_to_u32(row_idx);
    let raw = AirRaw::from_row(row, shared);

    let unq_key = build_unq_key_from_parts(
        SHEET,
        row_u32,
        raw.key.code.as_deref(),
        raw.key.num.as_deref(),
        raw.key.e_num.as_deref(),
    )?;

    Ok(ParsedAir {
        unq_key,
        excel_row: row_idx,
        data: AirDetail {
            passenger_name: raw.passenger_name,
            id_no: raw.id_no,
            depart: raw.depart,
            arrival: raw.arrive,
            carrier: raw.carrier,
            flight_no: raw.flight_no,
            flight_date: raw.flight_date,
            flight_time: raw.flight_time,
        },
    })
}
