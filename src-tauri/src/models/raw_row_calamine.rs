// src-tauri/src/models/raw_row.rs
use calamine::Data;
// use crate::excel_reader::CellValue;
use crate::utils::{opt_f64, opt_str, req_str};
// ─────────────────────────────────────────────────────────────
// 内部辅助函数
// ─────────────────────────────────────────────────────────────

// ─────────────────────────────────────────────────────────────
// 发票基础信息 sheet —— 发票头
// ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct HeaderRaw {
    pub code: Option<String>,
    pub num: Option<String>,
    pub e_num: Option<String>,
    pub seller_tax_no: String,
    pub seller_name: String,
    pub issue_date: String,
    pub amount: String,
    pub tax: String,
    pub total: String,
    pub source: String,
    pub inv_type: String,
    pub state: String,
    pub is_pos: String,
    pub risk: String,
    pub issuer: Option<String>,
    pub remark: Option<String>,
}

impl HeaderRaw {
    pub fn from_row(row: &[Data]) -> Result<Self, (usize, ())> {
        Ok(Self {
            code: opt_str(row, 1),
            num: opt_str(row, 2),
            e_num: opt_str(row, 3),
            seller_tax_no: req_str(row, 4)?,
            seller_name: req_str(row, 5)?,
            issue_date: req_str(row, 8)?,
            amount: req_str(row, 9)?,
            tax: req_str(row, 10)?,
            total: req_str(row, 11)?,
            source: req_str(row, 12)?,
            inv_type: req_str(row, 13)?,
            state: req_str(row, 14)?,
            is_pos: req_str(row, 15)?,
            risk: req_str(row, 16)?,
            issuer: opt_str(row, 17),
            remark: opt_str(row, 18),
        })
    }
}

// ─────────────────────────────────────────────────────────────
// 信息汇总表 sheet —— 发票明细
// ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct DetailRaw {
    pub code: Option<String>,
    pub num: Option<String>,
    pub e_num: Option<String>,
    pub tax_cls_code: Option<String>,
    pub special_biz: Option<String>,
    pub goods_name: String,
    pub spec: Option<String>,
    pub unit: Option<String>,
    pub quantity: Option<f64>,
    pub unit_price: Option<f64>,
    pub amount: String,
    pub tax_rate: Option<String>,
    pub tax: String,
    pub total: String,
    pub issuer: Option<String>,
    pub remark: Option<String>,
}

impl DetailRaw {
    pub fn from_row(row: &[Data]) -> Result<Self, (usize, ())> {
        Ok(Self {
            code: opt_str(row, 1),
            num: opt_str(row, 2),
            e_num: opt_str(row, 3),
            tax_cls_code: opt_str(row, 9),
            special_biz: opt_str(row, 10),
            goods_name: req_str(row, 11)?,
            spec: opt_str(row, 12),
            unit: opt_str(row, 13),
            quantity: opt_f64(row, 14),
            unit_price: opt_f64(row, 15),
            amount: req_str(row, 16)?,
            tax_rate: opt_str(row, 17),
            tax: req_str(row, 18)?,
            total: req_str(row, 19)?,
            issuer: opt_str(row, 25),
            remark: opt_str(row, 26),
        })
    }
}

// ─────────────────────────────────────────────────────────────
// 特种行业 sheet 公共前缀
// ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct SheetKeyRaw {
    pub code: Option<String>,
    pub num: Option<String>,
    pub e_num: Option<String>,
}

impl SheetKeyRaw {
    #[inline]
    pub fn from_row(row: &[Data]) -> Self {
        Self {
            code: opt_str(row, 1),
            num: opt_str(row, 2),
            e_num: opt_str(row, 3),
        }
    }
}

// ─────────────────────────────────────────────────────────────
// 铁路电子客票
// ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct RailwayRaw {
    pub key: SheetKeyRaw,
    pub passenger_name: Option<String>,
    pub id_no: Option<String>,
    pub depart: Option<String>,
    pub arrive: Option<String>,
    pub train_no: Option<String>,
    pub travel_date: Option<String>,
    pub travel_time: Option<String>,
}

impl RailwayRaw {
    pub fn from_row(row: &[Data]) -> Self {
        Self {
            key: SheetKeyRaw::from_row(row),
            passenger_name: opt_str(row, 16),
            id_no: opt_str(row, 17),
            depart: opt_str(row, 18),
            arrive: opt_str(row, 19),
            train_no: opt_str(row, 20),
            travel_date: opt_str(row, 21),
            travel_time: opt_str(row, 22),
        }
    }
}

// ─────────────────────────────────────────────────────────────
// 旅客运输服务
// ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct PassengerRaw {
    pub key: SheetKeyRaw,
    pub traveler: Option<String>,
    pub id_no: Option<String>,
    pub travel_date: Option<String>,
    pub depart: Option<String>,
    pub arrive: Option<String>,
    pub seat_class: Option<String>,
    pub transport_type: Option<String>,
}

impl PassengerRaw {
    pub fn from_row(row: &[Data]) -> Self {
        Self {
            key: SheetKeyRaw::from_row(row),
            traveler: opt_str(row, 16),
            id_no: opt_str(row, 17),
            travel_date: opt_str(row, 18),
            depart: opt_str(row, 19),
            arrive: opt_str(row, 20),
            seat_class: opt_str(row, 21),
            transport_type: opt_str(row, 22),
        }
    }
}

// ─────────────────────────────────────────────────────────────
// 通行费数电发票
// ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct TollRaw {
    pub key: SheetKeyRaw,
    pub plate_no: Option<String>,
    pub vehicle_type: Option<String>,
    pub toll_date_from: Option<String>,
    pub toll_date_to: Option<String>,
}

impl TollRaw {
    pub fn from_row(row: &[Data]) -> Self {
        Self {
            key: SheetKeyRaw::from_row(row),
            plate_no: opt_str(row, 16),
            vehicle_type: opt_str(row, 17),
            toll_date_from: opt_str(row, 18),
            toll_date_to: opt_str(row, 19),
        }
    }
}

// ─────────────────────────────────────────────────────────────
// 建筑服务
// ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct BuildingRaw {
    pub key: SheetKeyRaw,
    pub cross_region_no: Option<String>,
    pub service_location: Option<String>,
    pub project_name: Option<String>,
}

impl BuildingRaw {
    pub fn from_row(row: &[Data]) -> Self {
        Self {
            key: SheetKeyRaw::from_row(row),
            cross_region_no: opt_str(row, 16),
            service_location: opt_str(row, 17),
            project_name: opt_str(row, 18),
        }
    }
}

// ─────────────────────────────────────────────────────────────
// 货物运输服务
// ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct CargoRaw {
    pub key: SheetKeyRaw,
    pub transport_type: Option<String>,
    pub vehicle_no: Option<String>,
    pub depart: Option<String>,
    pub arrive: Option<String>,
    pub goods_name: Option<String>,
}

impl CargoRaw {
    pub fn from_row(row: &[Data]) -> Self {
        Self {
            key: SheetKeyRaw::from_row(row),
            transport_type: opt_str(row, 16),
            vehicle_no: opt_str(row, 17),
            depart: opt_str(row, 18),
            arrive: opt_str(row, 19),
            goods_name: opt_str(row, 20),
        }
    }
}

// ─────────────────────────────────────────────────────────────
// 不动产经营租赁服务
// ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct RealEstateRaw {
    pub key: SheetKeyRaw,
    pub property_cert_no: Option<String>,
    pub plate_no: Option<String>,
}

impl RealEstateRaw {
    pub fn from_row(row: &[Data]) -> Self {
        Self {
            key: SheetKeyRaw::from_row(row),
            property_cert_no: opt_str(row, 16),
            plate_no: opt_str(row, 17),
        }
    }
}

// ─────────────────────────────────────────────────────────────
// 航空运输电子客票行程单
// ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct AirRaw {
    pub key: SheetKeyRaw,
    pub passenger_name: Option<String>,
    pub id_no: Option<String>,
    pub depart: Option<String>,
    pub arrive: Option<String>,
    pub carrier: Option<String>,
    pub flight_no: Option<String>,
    pub flight_date: Option<String>,
    pub flight_time: Option<String>,
}

impl AirRaw {
    pub fn from_row(row: &[Data]) -> Self {
        Self {
            key: SheetKeyRaw::from_row(row),
            passenger_name: opt_str(row, 16),
            id_no: opt_str(row, 17),
            depart: opt_str(row, 18),
            arrive: opt_str(row, 19),
            carrier: opt_str(row, 20),
            flight_no: opt_str(row, 21),
            flight_date: opt_str(row, 22),
            flight_time: opt_str(row, 23),
        }
    }
}
