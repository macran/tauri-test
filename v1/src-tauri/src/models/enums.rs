// src-tauri/src/models/enums.rs
//
// 所有枚举统一策略：
//   - #[repr(u8)]          → 直接 `as u8` 存库，无需手写 to_u8()
//   - #[derive(EnumString)] → 自动实现 FromStr，直接 "正常".parse::<InvState>() 解析 Excel 字符串
//   - #[strum(serialize)]  → 指定每个变体对应的中文字符串
//   - specta::Type         → 自动生成 TypeScript 类型定义
//
// 注意：#[repr(u8)] 与 specta::Type 同时使用时，specta 会将其视为普通枚举（string union），
// 不影响前端类型安全，DB 层用 `variant as u8` 存整数即可。

use serde::{Deserialize, Serialize};
use specta::Type;
use strum::{Display, EnumString};

// ─────────────────────────────────────────────────────────────
// 发票来源
// ─────────────────────────────────────────────────────────────
#[repr(u8)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type, EnumString)]
#[serde(rename_all = "camelCase")]
pub enum InvSource {
    #[strum(serialize = "电子发票服务平台")]
    EInvPlatform = 1,

    #[strum(serialize = "增值税发票管理系统")]
    VatSystem = 2,
}

// ─────────────────────────────────────────────────────────────
// 发票票种
// ─────────────────────────────────────────────────────────────
#[repr(u8)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type, EnumString)]
#[serde(rename_all = "camelCase")]
pub enum InvType {
    #[strum(serialize = "数电发票（普通发票）")]
    EDigitalNormal = 1,

    #[strum(serialize = "数电发票（增值税专用发票）")]
    EDigitalSpecial = 2,

    #[strum(serialize = "增值税专用发票")]
    VatSpecial = 3,

    #[strum(serialize = "增值税电子普通发票")]
    VatENormal = 4,

    #[strum(serialize = "数电纸质发票（普通发票）")]
    EDigitalPaperNormal = 5,

    #[strum(serialize = "增值税普通发票")]
    VatNormal = 6,

    #[strum(serialize = "数电纸质发票（增值税专用发票）")]
    EDigitalPaperSpecial = 7,

    #[strum(serialize = "增值税普通发票（卷式）")]
    VatRoll = 8,

    #[strum(serialize = "道路通行费电子普通发票")]
    TollENormal = 9,

    #[strum(serialize = "数电发票（铁路电子客票）")]
    EDigitalRailway = 10,

    #[strum(serialize = "数电发票（航空运输电子客票行程单）")]
    EDigitalAir = 11,

    #[strum(serialize = "数电发票（通行费发票）")]
    EDigitalToll = 12,
}

// ─────────────────────────────────────────────────────────────
// 发票状态
// ─────────────────────────────────────────────────────────────
#[repr(u8)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type, EnumString)]
#[serde(rename_all = "camelCase")]
pub enum InvState {
    #[strum(serialize = "正常")]
    Normal = 1,

    #[strum(serialize = "已作废")]
    Voided = 2,

    #[strum(serialize = "已红冲-全额")]
    ReversedFull = 3,

    #[strum(serialize = "已红冲-部分")]
    ReversedPartial = 4,

    #[strum(serialize = "异常")]
    Abnormal = 5,
}

// ─────────────────────────────────────────────────────────────
// 是否正数发票
// ─────────────────────────────────────────────────────────────
#[repr(u8)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type, EnumString)]
#[serde(rename_all = "camelCase")]
pub enum IsPositive {
    /// 否 = 0（红字发票）
    #[strum(serialize = "否")]
    No = 0,

    /// 是 = 1（蓝字发票）
    #[strum(serialize = "是")]
    Yes = 1,
}

// ─────────────────────────────────────────────────────────────
// 发票风险等级
// ─────────────────────────────────────────────────────────────
#[repr(u8)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type, EnumString)]
#[serde(rename_all = "camelCase")]
pub enum RiskLevel {
    #[strum(serialize = "正常")]
    Normal = 1,

    #[strum(serialize = "疑点")]
    Suspicious = 2,

    #[strum(serialize = "风险")]
    Abnormal = 3,

    #[strum(serialize = "高危")]
    HighRisk = 4,
}

//认证清单中的风险状态

#[repr(u8)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type, EnumString)]
#[serde(rename_all = "camelCase")]
pub enum CertRiskStatus {
    #[strum(serialize = "无风险")]
    NoRisk = 0,
    #[strum(serialize = "蓝色预警")]
    Suspicious = 1,
    #[strum(serialize = "黄色预警")]
    Risk = 2,
    #[strum(serialize = "红色预警")]
    HighRisk = 3,
}

impl CertRiskStatus {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.trim() {
            "无风险" => Some(Self::NoRisk),
            "蓝色预警" => Some(Self::Suspicious),
            "黄色预警" => Some(Self::Risk),
            "红色预警" => Some(Self::HighRisk),
            _ => None,
        }
    }
}
// ─────────────────────────────────────────────────────────────
// 特定业务类型（信息汇总表专用）
// ─────────────────────────────────────────────────────────────
#[repr(u8)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type, EnumString)]
#[serde(rename_all = "camelCase")]
pub enum SpecialBizType {
    #[strum(serialize = "铁路电子客票")]
    Railway = 1,

    #[strum(serialize = "旅客运输服务发票")]
    PassengerTransport = 2,

    #[strum(serialize = "通行费")]
    Toll = 3,

    #[strum(serialize = "成品油发票")]
    RefinedOil = 4,

    #[strum(serialize = "农产品")]
    AgriculturalProduct = 5,

    #[strum(serialize = "货物运输服务发票")]
    CargoTransport = 6,

    #[strum(serialize = "不动产经营租赁服务")]
    RealEstateRental = 7,

    #[strum(serialize = "航空运输电子客票行程单")]
    AirTicket = 8,

    #[strum(serialize = "建筑服务发票")]
    Construction = 9,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Type, EnumString, Display)]
#[serde(rename_all = "camelCase")]
pub enum ApInvType {
    #[strum(
        serialize = "增票",
        serialize = "增专",
        serialize = "专票",
        serialize = "增值税专用发票"
    )]
    ZengPiao = 0,

    #[strum(
        serialize = "普票",
        serialize = "普通发票",
        serialize = "增值税普通发票"
    )]
    PuPiao = 1,

    #[strum(serialize = "红票", serialize = "红字发票")]
    HongPiao = 2,

    #[strum(serialize = "收据")]
    ShouJu = 3,

    #[strum(serialize = "票据")]
    PiaoJu = 4,

    #[strum(serialize = "通知书", serialize = "行程单", serialize = "通知单")]
    JieSuanDan = 5,
}

impl ApInvType {
    pub fn requires_header(&self) -> bool {
        matches!(self, Self::ZengPiao | Self::PuPiao)
    }

    pub fn needs_inv_type_check(&self) -> bool {
        matches!(self, Self::ZengPiao)
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Type, EnumString, Display)]
#[serde(rename_all = "camelCase")]
pub enum TaxCertified {
    #[strum(serialize = "否")]
    No = 0,

    #[strum(serialize = "是")]
    Yes = 1,
}
