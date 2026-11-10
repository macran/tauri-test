// src-tauri/src/features/invoice_import/models.rs
//
// 全量发票导入专用的数据结构：
//   - DTO（前端交互用，camelCase，标注 specta::Type）
//   - 插入数据库用的结构体（枚举字段已转换为 u8，金额字段已转换为以分为单位的 i64）
//   - Raw 结构体（直接映射 Excel 列，供各 sheet 的 parser 使用）

use crate::excel_reader::CellValue;
use crate::utils::{opt_f64, opt_str, req_amount_str, req_str};
use serde::{Deserialize, Serialize};
use specta::Type;

// ─────────────────────────────────────────────
// DTO：前端交互用
// ─────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CellError {
    /// Excel 行号（从 1 开始，包含表头行）
    pub row: u32,
    /// 列名称（中文列名）
    pub col: String,
    /// 错误描述
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SheetImportResult {
    /// Sheet 名称
    pub sheet_name: String,
    /// 是否被识别（如果 Excel 中不存在该 sheet 则为 false）
    pub recognized: bool,
    /// 成功插入行数
    pub inserted: u32,
    /// 解析错误行数（这些行未入库）
    pub error_count: u32,
    /// 错误详情列表（定位到具体单元格）
    pub errors: Vec<CellError>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ImportSummary {
    /// 每个 sheet 的详细结果
    pub sheets: Vec<SheetImportResult>,
    /// 是否成功提交（false 表示回滚）
    pub committed: bool,
    /// 如果回滚，这里说明原因
    pub rollback_reason: Option<String>,
    /// 导入的 sellers 数量（新增 + 更新）
    pub seller_count: u32,
    /// 发票头成功插入数
    pub header_inserted: u32,
    /// 发票明细成功插入数
    pub detail_inserted: u32,
}

// ─────────────────────────────────────────────────────────────
// 插入数据库用的结构体
//
// 设计原则：
//   - header_id 和 line_no 不放在各 Detail 结构体中。
//     header_id 在 repository 层通过 unq_map 查找后注入；
//     line_no   在 repository 层由 compute_line_nos() 计算后注入。
//   - 各 Detail 结构体只包含该表自身的业务字段，与建表语句一一对应。
// ─────────────────────────────────────────────────────────────

/// sellers 表
#[allow(dead_code)] // 目前由 SQL 直接写 sellers，这个结构暂未使用
#[derive(Debug, Clone)]
pub struct Seller {
    /// 销方纳税人识别号（UNIQUE）
    pub tax_no: String,
    /// 销方名称
    pub name: String,
}

/// invoice_headers 表
///
/// 建表字段（不含生成列 inv_unq_key）：
///   id, code, num, e_num, seller_id, issue_date,
///   amount, tax, total,
///   source, inv_type, state, is_pos, risk,
///   issuer, remark, rev_org_key, red_notice_no
#[derive(Debug, Clone)]
#[allow(dead_code)] // seller_id 在入库时才确定，构造时填 0
pub struct InvoiceHeader {
    pub code: Option<String>,
    pub num: Option<String>,
    pub e_num: Option<String>,
    /// 占位符，在 repository 层 upsert_seller 后填充
    pub seller_id: i64,
    pub issue_date: String,
    /// 金额，分
    pub amount: i64,
    /// 税额，分
    pub tax: i64,
    /// 价税合计，分
    pub total: i64,
    /// 发票来源枚举 → u8
    pub source: u8,
    /// 发票票种枚举 → u8
    pub inv_type: u8,
    /// 发票状态枚举 → u8
    pub state: u8,
    /// 是否正数发票枚举 → u8
    pub is_pos: u8,
    /// 发票风险等级枚举 → u8
    pub risk: u8,
    pub issuer: Option<String>,
    pub remark: Option<String>,
    /// 从备注提取的红冲原始 unq_key（红字发票专用）
    pub rev_org_key: Option<String>,
    pub red_notice_no: Option<String>,
}

/// ParsedHeader —— parser 层产出的发票头中间结构体
#[derive(Debug, Clone)]
pub struct ParsedHeader {
    /// 插库用的发票头数据
    pub data: InvoiceHeader,
    /// 内存 lookup 用，不插库（数据库由生成列自动计算）
    pub unq_key: String,
    /// upsert sellers 用
    pub seller_tax_no: String,
    /// upsert sellers 用
    pub seller_name: String,
    /// 错误定位用（Excel 行号，从 2 开始）
    pub excel_row: usize,
}

/// invoice_details 表
///
/// 建表字段：
///   id, header_id, line_no,
///   tax_code, biz_type, goods_category,
///   goods_name, spec_model, unit,
///   quantity, unit_price,
///   amount, tax_rate, tax, total
///
/// 注意：header_id、line_no 不在此结构体中，由 repository 层注入。
#[derive(Debug, Clone)]
pub struct InvoiceDetail {
    /// 税收分类编码（tax_code）
    pub tax_code: Option<String>,
    /// 特定业务类型（biz_type），已转换为 u8
    pub biz_type: Option<u8>,
    /// 货物或劳务、服务类别（goods_category）
    pub goods_category: Option<String>,
    /// 货物或应税劳务、服务名称（goods_name），NOT NULL
    pub goods_name: String,
    /// 规格型号（spec_model）
    pub spec_model: Option<String>,
    /// 单位（unit）
    pub unit: Option<String>,
    /// 数量（quantity）
    pub quantity: Option<f64>,
    /// 单价（unit_price）
    pub unit_price: Option<f64>,
    /// 金额，分（amount），NOT NULL
    pub amount: i64,
    /// 税率（tax_rate），TEXT 存储，如 "13%"
    pub tax_rate: Option<String>,
    /// 税额，分（tax），NOT NULL
    pub tax: i64,
    /// 价税合计，分（total），NOT NULL
    pub total: i64,
}

/// ParsedDetail —— parser 层产出的信息汇总表明细中间结构体
#[derive(Debug, Clone)]
pub struct ParsedInvoiceDetail {
    /// 内存 lookup 用（对应 invoice_headers.inv_unq_key）
    pub unq_key: String,
    /// 插库用的明细数据
    pub data: InvoiceDetail,
    /// 错误定位用（Excel 行号，从 2 开始）
    pub excel_row: usize,
}

/// detail_railway 表（铁路电子客票）
#[derive(Debug, Clone)]
pub struct RailwayDetail {
    pub passenger_name: Option<String>, // 旅客姓名
    pub id_no: Option<String>,          // 有效身份证号
    pub depart: Option<String>,         // 出发地
    pub arrival: Option<String>,        // 到达地
    pub train_no: Option<String>,       // 出行车次
    pub travel_date: Option<String>,    // 日期
    pub travel_time: Option<String>,    // 时间
}

#[derive(Debug, Clone)]
pub struct ParsedRailway {
    pub unq_key: String,
    pub excel_row: usize,
    pub data: RailwayDetail,
}

/// detail_passenger_transport 表（旅客运输服务）
#[derive(Debug, Clone)]
pub struct PassengerDetail {
    pub traveler_name: Option<String>,  // 出行人
    pub id_no: Option<String>,          // 有效身份证号
    pub travel_date: Option<String>,    // 出行日期
    pub depart: Option<String>,         // 出发地
    pub arrival: Option<String>,        // 到达地
    pub seat_class: Option<String>,     // 等级
    pub transport_type: Option<String>, // 交通工具类型
}

#[derive(Debug, Clone)]
pub struct ParsedPassenger {
    pub unq_key: String,
    pub excel_row: usize,
    pub data: PassengerDetail,
}

/// detail_toll 表（通行费数电发票）
#[derive(Debug, Clone)]
pub struct TollDetail {
    pub plate_no: Option<String>,        // 车牌号码
    pub vehicle_type: Option<String>,    // 车辆类型
    pub pass_start_date: Option<String>, // 通行日期起
    pub pass_end_date: Option<String>,   // 通行日期止
}

#[derive(Debug, Clone)]
pub struct ParsedToll {
    pub unq_key: String,
    pub excel_row: usize,
    pub data: TollDetail,
}

/// detail_building 表（建筑服务）
#[derive(Debug, Clone)]
pub struct BuildingDetail {
    pub cross_region_no: Option<String>, // 跨区域涉税事项报验管理编号
    pub service_location: Option<String>, // 建筑服务发生地
    pub project_name: Option<String>,    // 建筑项目名称
}

#[derive(Debug, Clone)]
pub struct ParsedBuilding {
    pub unq_key: String,
    pub excel_row: usize,
    pub data: BuildingDetail,
}

/// detail_cargo_transport 表（货物运输服务）
#[derive(Debug, Clone)]
pub struct CargoDetail {
    pub vehicle_kind: Option<String>,     // 运输工具种类
    pub vehicle_plate_no: Option<String>, // 运输工具牌号
    pub depart: Option<String>,           // 起运地
    pub arrival: Option<String>,          // 到达地
    pub cargo_name: Option<String>,       // 运输货物名称
}

#[derive(Debug, Clone)]
pub struct ParsedCargo {
    pub unq_key: String,
    pub excel_row: usize,
    pub data: CargoDetail,
}

/// detail_real_estate_rent 表（不动产经营租赁服务）
#[derive(Debug, Clone)]
pub struct RealEstateDetail {
    pub cert_no: Option<String>,  // 产权证书/不动产权证号
    pub plate_no: Option<String>, // 车牌号
}

#[derive(Debug, Clone)]
pub struct ParsedRealEstate {
    pub unq_key: String,
    pub excel_row: usize,
    pub data: RealEstateDetail,
}

/// detail_air_transport 表（航空运输电子客票行程单）
#[derive(Debug, Clone)]
pub struct AirDetail {
    pub passenger_name: Option<String>, // 旅客姓名
    pub id_no: Option<String>,          // 有效身份证号
    pub depart: Option<String>,         // 出发地
    pub arrival: Option<String>,        // 到达地
    pub carrier: Option<String>,        // 承运人
    pub flight_no: Option<String>,      // 航班号
    pub flight_date: Option<String>,    // 日期
    pub flight_time: Option<String>,    // 时间
}

#[derive(Debug, Clone)]
pub struct ParsedAir {
    pub unq_key: String,
    pub excel_row: usize,
    pub data: AirDetail,
}

// ─────────────────────────────────────────────────────────────
// Raw 结构体：直接映射 Excel 列（信息汇总表 + 7 个特种行业表）
// ─────────────────────────────────────────────────────────────

/// 信息汇总表 sheet —— 发票明细
#[derive(Debug, Clone)]
#[allow(dead_code)] // issuer / remark 来自 Excel，目前只有一部分被读取
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

/// 特种行业 sheet 公共前缀
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

/// 铁路电子客票
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

/// 旅客运输服务
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

/// 通行费数电发票
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

/// 建筑服务
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

/// 货物运输服务
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

/// 不动产经营租赁服务
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

/// 航空运输电子客票行程单
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
