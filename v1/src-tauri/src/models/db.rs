// src-tauri/src/models/db.rs
//
// 插入数据库用的结构体。
// 所有枚举字段已转换为 u8，金额字段已转换为以分为单位的 i64。
//
// 设计原则：
//   - header_id 和 line_no 不放在各 Detail 结构体中。
//     header_id 在 operations 层通过 unq_map 查找后注入；
//     line_no   在 operations 层由 compute_line_nos() 计算后注入。
//   - 各 Detail 结构体只包含该表自身的业务字段，与建表语句一一对应。

// ─────────────────────────────────────────────────────────────
// sellers 表
// ─────────────────────────────────────────────────────────────
#[derive(Debug, Clone)]
pub struct Seller {
    /// 销方纳税人识别号（UNIQUE）
    pub tax_no: String,
    /// 销方名称
    pub name: String,
}

// ─────────────────────────────────────────────────────────────
// invoice_headers 表
//
// 建表字段（不含生成列 inv_unq_key）：
//   id, code, num, e_num, seller_id, issue_date,
//   amount, tax, total,
//   source, inv_type, state, is_pos, risk,
//   issuer, remark, rev_org_key, batch_no
// ─────────────────────────────────────────────────────────────
#[derive(Debug, Clone)]
pub struct InvoiceHeader {
    pub code: Option<String>,
    pub num: Option<String>,
    pub e_num: Option<String>,
    /// 占位符，在 operations 层 upsert_seller 后填充
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
    /// 本次导入批次号（Unix 时间戳秒级）
    
    pub batch_no: i32,
}

// ─────────────────────────────────────────────────────────────
// ParsedHeader —— parser 层产出的发票头中间结构体
// ─────────────────────────────────────────────────────────────
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

// ─────────────────────────────────────────────────────────────
// invoice_details 表
//
// 建表字段：
//   id, header_id, line_no,
//   tax_code, biz_type, goods_category,
//   goods_name, spec_model, unit,
//   quantity, unit_price,
//   amount, tax_rate, tax, total
//
// 注意：header_id、line_no 不在此结构体中，由 operations 层注入。
// ─────────────────────────────────────────────────────────────
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
    /// 数量（quantity），TEXT 存储（保留原始格式）
    pub quantity: Option<f64>,
    /// 单价（unit_price），TEXT 存储（保留原始格式）
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

// ─────────────────────────────────────────────────────────────
// ParsedDetail —— parser 层产出的信息汇总表明细中间结构体
// ─────────────────────────────────────────────────────────────
#[derive(Debug, Clone)]
pub struct ParsedInvoiceDetail {
    /// 内存 lookup 用（对应 invoice_headers.inv_unq_key）
    pub unq_key: String,
    /// 插库用的明细数据
    pub data: InvoiceDetail,
    /// 错误定位用（Excel 行号，从 2 开始）
    pub excel_row: usize,
}

// ─────────────────────────────────────────────────────────────
// detail_railway 表（铁路电子客票）
//
// 建表字段：
//   id, header_id, line_no,
//   passenger_name, passenger_id_no,
//   departure_place, arrival_place,
//   train_no, travel_date, travel_time
//
// 注意：header_id、line_no 不在此结构体中，由 operations 层注入。
// ─────────────────────────────────────────────────────────────
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

// ─────────────────────────────────────────────────────────────
// detail_passenger_transport 表（旅客运输服务）
//
// 建表字段：
//   id, header_id, line_no,
//   traveler_name, traveler_id_no, travel_date,
//   departure_place, arrival_place, level, vehicle_type
//
// 注意：header_id、line_no 不在此结构体中，由 operations 层注入。
// ─────────────────────────────────────────────────────────────
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

// ─────────────────────────────────────────────────────────────
// detail_toll 表（通行费数电发票）
//
// 建表字段：
//   id, header_id, line_no,
//   plate_no, vehicle_type,
//   pass_start_date, pass_end_date
//
// 注意：header_id、line_no 不在此结构体中，由 operations 层注入。
// ─────────────────────────────────────────────────────────────
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

// ─────────────────────────────────────────────────────────────
// detail_building 表（建筑服务）
//
// 建表字段：
//   id, header_id, line_no,
//   cross_region_tax_no, building_service_location, building_project_name
//
// 注意：header_id、line_no 不在此结构体中，由 operations 层注入。
// ─────────────────────────────────────────────────────────────
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

// ─────────────────────────────────────────────────────────────
// detail_cargo_transport 表（货物运输服务）
//
// 建表字段：
//   id, header_id, line_no,
//   vehicle_kind, vehicle_plate_no,
//   departure_place, arrival_place, cargo_name
//
// 注意：header_id、line_no 不在此结构体中，由 operations 层注入。
// ─────────────────────────────────────────────────────────────
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

// ─────────────────────────────────────────────────────────────
// detail_real_estate_rent 表（不动产经营租赁服务）
//
// 建表字段：
//   id, header_id, line_no,
//   property_cert_no, plate_no
//
// 注意：header_id、line_no 不在此结构体中，由 operations 层注入。
// ─────────────────────────────────────────────────────────────
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

// ─────────────────────────────────────────────────────────────
// detail_air_transport 表（航空运输电子客票行程单）
//
// 建表字段：
//   id, header_id, line_no,
//   passenger_name, passenger_id_no,
//   departure_place, arrival_place,
//   carrier, flight_no, flight_date, flight_time
//
// 注意：header_id、line_no 不在此结构体中，由 operations 层注入。
// ─────────────────────────────────────────────────────────────
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

#[derive(Debug, Clone)]
pub struct CertifiedList {
    /// 关联 invoice_headers.id
    pub header_id: i64,
    /// 转内销证明编号（可为空）
    pub domestic_proof_no: Option<String>,
    /// 有效抵扣税额（分，NOT NULL）
    pub valid_deduct_amount: i64,
    /// 票种标签（NOT NULL）
    pub invoice_tag: String,
    /// 勾选时间（NOT NULL）
    pub check_time: String,
    /// 风险状态整型（NOT NULL）
    pub risk_status: u8,
    /// 税款所属期（如 "202604"）
    pub tax_period: String,
    /// 是否为差旅费（固定 0），voucher_no / posted_date 暂不插入
    pub is_travel_expense: u8,
    pub voucher_no: Option<String>,
    pub posted_date: Option<String>,
}
