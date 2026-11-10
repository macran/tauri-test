// src-tauri/src/models/raw_row.rs
use crate::excel_reader::CellValue;
use crate::utils::{opt_f64, opt_str, req_amount_str, req_str};
use calamine::Data;
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
    pub fn from_row(row: &[CellValue], shared: &[String]) -> Result<Self, (usize, ())> {
        Ok(Self {
            code: opt_str(row, 1, shared),
            num: opt_str(row, 2, shared),
            e_num: opt_str(row, 3, shared),
            seller_tax_no: req_str(row, 4, shared)?,
            seller_name: req_str(row, 5, shared)?,
            issue_date: req_str(row, 8, shared)?,
            amount: req_amount_str(row, 9, shared)?,
            tax: req_amount_str(row, 10, shared)?,
            total: req_amount_str(row, 11, shared)?,
            source: req_str(row, 12, shared)?,
            inv_type: req_str(row, 13, shared)?,
            state: req_str(row, 14, shared)?,
            is_pos: req_str(row, 15, shared)?,
            risk: req_str(row, 16, shared)?,
            issuer: opt_str(row, 17, shared),
            remark: opt_str(row, 18, shared),
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
    pub fn from_row(row: &[CellValue], shared: &[String]) -> Result<Self, (usize, ())> {
        Ok(Self {
            code: opt_str(row, 1, shared),
            num: opt_str(row, 2, shared),
            e_num: opt_str(row, 3, shared),
            tax_cls_code: opt_str(row, 9, shared),
            special_biz: opt_str(row, 10, shared),
            goods_name: req_str(row, 11, shared)?,
            spec: opt_str(row, 12, shared),
            unit: opt_str(row, 13, shared),
            quantity: opt_f64(row, 14, shared),
            unit_price: opt_f64(row, 15, shared),
            amount: req_amount_str(row, 16, shared)?,
            tax_rate: opt_str(row, 17, shared),
            tax: req_amount_str(row, 18, shared)?,
            total: req_amount_str(row, 19, shared)?,
            issuer: opt_str(row, 25, shared),
            remark: opt_str(row, 26, shared),
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
    pub fn from_row(row: &[CellValue], shared: &[String]) -> Self {
        Self {
            code: opt_str(row, 1, shared),
            num: opt_str(row, 2, shared),
            e_num: opt_str(row, 3, shared),
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
    pub fn from_row(row: &[CellValue], shared: &[String]) -> Self {
        Self {
            key: SheetKeyRaw::from_row(row, shared),
            passenger_name: opt_str(row, 16, shared),
            id_no: opt_str(row, 17, shared),
            depart: opt_str(row, 18, shared),
            arrive: opt_str(row, 19, shared),
            train_no: opt_str(row, 20, shared),
            travel_date: opt_str(row, 21, shared),
            travel_time: opt_str(row, 22, shared),
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
    pub fn from_row(row: &[CellValue], shared: &[String]) -> Self {
        Self {
            key: SheetKeyRaw::from_row(row, shared),
            traveler: opt_str(row, 16, shared),
            id_no: opt_str(row, 17, shared),
            travel_date: opt_str(row, 18, shared),
            depart: opt_str(row, 19, shared),
            arrive: opt_str(row, 20, shared),
            seat_class: opt_str(row, 21, shared),
            transport_type: opt_str(row, 22, shared),
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
    pub fn from_row(row: &[CellValue], shared: &[String]) -> Self {
        Self {
            key: SheetKeyRaw::from_row(row, shared),
            plate_no: opt_str(row, 16, shared),
            vehicle_type: opt_str(row, 17, shared),
            toll_date_from: opt_str(row, 18, shared),
            toll_date_to: opt_str(row, 19, shared),
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
    pub fn from_row(row: &[CellValue], shared: &[String]) -> Self {
        Self {
            key: SheetKeyRaw::from_row(row, shared),
            cross_region_no: opt_str(row, 16, shared),
            service_location: opt_str(row, 17, shared),
            project_name: opt_str(row, 18, shared),
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
    pub fn from_row(row: &[CellValue], shared: &[String]) -> Self {
        Self {
            key: SheetKeyRaw::from_row(row, shared),
            transport_type: opt_str(row, 16, shared),
            vehicle_no: opt_str(row, 17, shared),
            depart: opt_str(row, 18, shared),
            arrive: opt_str(row, 19, shared),
            goods_name: opt_str(row, 20, shared),
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
    pub fn from_row(row: &[CellValue], shared: &[String]) -> Self {
        Self {
            key: SheetKeyRaw::from_row(row, shared),
            property_cert_no: opt_str(row, 16, shared),
            plate_no: opt_str(row, 17, shared),
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
    pub fn from_row(row: &[CellValue], shared: &[String]) -> Self {
        Self {
            key: SheetKeyRaw::from_row(row, shared),
            passenger_name: opt_str(row, 16, shared),
            id_no: opt_str(row, 17, shared),
            depart: opt_str(row, 18, shared),
            arrive: opt_str(row, 19, shared),
            carrier: opt_str(row, 20, shared),
            flight_no: opt_str(row, 21, shared),
            flight_date: opt_str(row, 22, shared),
            flight_time: opt_str(row, 23, shared),
        }
    }
}

// 认证清单 sheet
#[derive(Debug, Clone)]
pub struct CertifiedRaw {
    /// 数电发票号码（用于 build_unq_key）
    pub e_num: Option<String>,
    /// 发票代码（用于 build_unq_key）
    pub code: Option<String>,
    /// 发票号码（用于 build_unq_key）
    pub num: Option<String>,
    /// 转内销证明编号（可为空）
    pub domestic_proof_no: Option<String>,
    /// 开票日期（仅用于 header 未找到时的展示，不参与入库）
    pub issue_date: Option<String>,
    /// 销售方纳税人识别号（仅用于 header 未找到时的展示，不参与入库）
    pub seller_tax_no: Option<String>,
    /// 销售方纳税人名称（仅用于 header 未找到时的展示，不参与入库）
    pub seller_name: Option<String>,
    /// 有效抵扣税额（元，字符串，后续转分）
    pub valid_deduct_amount: String,
    /// 票种标签
    pub invoice_tag: String,
    /// 勾选时间
    pub check_time: String,
    /// 风险状态原始字符串（后续枚举解析）
    pub risk_status_raw: String,
}

impl CertifiedRaw {
    pub fn from_row(row: &[Data]) -> Result<Self, (usize, ())> {
        Ok(Self {
            e_num: data_to_opt_str(row, 4),
            code: data_to_opt_str(row, 5),
            num: data_to_opt_str(row, 6),
            domestic_proof_no: data_to_opt_str(row, 3),
            issue_date: data_to_opt_str(row, 7),
            seller_tax_no: data_to_opt_str(row, 8),
            seller_name: data_to_opt_str(row, 9),
            valid_deduct_amount: data_to_req_str(row, 12).ok_or((12, ()))?,
            invoice_tag: data_to_req_str(row, 14).ok_or((14, ()))?,
            check_time: data_to_req_str(row, 16).ok_or((16, ()))?,
            risk_status_raw: data_to_req_str(row, 18).ok_or((18, ()))?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct ParsedCertifiedRow {
    /// 用于查找 invoice_headers.id 的唯一键
    pub inv_unq_key: String,
    /// Excel 行号（1-based，含表头行，用于错误定位）
    pub excel_row: u32,
    /// 入库数据
    pub data: CertifiedRaw,
}

// ─────────────────────────────────────────────────────────────
// raw_row 内部辅助函数（仅供本文件使用）
// 使用 calamine::Data（0.34+）
// ─────────────────────────────────────────────────────────────

/// 取可选字符串：空/Empty 返回 None
fn data_to_opt_str(row: &[Data], idx: usize) -> Option<String> {
    match row.get(idx)? {
        Data::String(s) => {
            let t = s.trim();
            if t.is_empty() {
                None
            } else {
                Some(t.to_string())
            }
        }
        Data::Int(n) => Some(n.to_string()),
        Data::Float(f) => {
            if f.fract() == 0.0 {
                Some(format!("{}", *f as i64))
            } else {
                Some(f.to_string())
            }
        }
        Data::Bool(b) => Some(b.to_string()),
        Data::Empty => None,
        _ => None,
    }
}

/// 取必填字符串：空/Empty 返回 None，由调用方转 (usize, ())
fn data_to_req_str(row: &[Data], idx: usize) -> Option<String> {
    data_to_opt_str(row, idx)
}
