// src-tauri/src/utils.rs

// use calamine::Data;
use crate::excel_reader::{CellValue, SheetRow};
use calamine::Data;
use chrono::{Duration, NaiveDate};
use regex::Regex;
use std::sync::LazyLock;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RedInvoiceReference {
    /// 被红冲的蓝字发票标识。
    ///
    /// 数电发票：20 位发票号码
    /// 传统发票：发票代码-发票号码
    pub blue_invoice_key: String,

    /// 红字发票信息确认单编号或红冲通知单编号。
    pub red_notice_no: Option<String>,
}


static INVOICE_PATTERNS: LazyLock<(Regex, Regex, Regex)> = LazyLock::new(|| {
    (
        // 模式1: 传统发票（先号码后代码）- 被(红冲|冲红)蓝字发票
        Regex::new(r"被(?:红冲|冲红)蓝字发票[号號]码[：:]\s*(\d{8}).*?被(?:红冲|冲红)蓝字发票代码[：:]\s*(\d{10,12})").unwrap(),
        // 模式2: 传统发票（先代码后号码）- 对应正数发票
        Regex::new(r"对应正数发票代码[：:]\s*(\d{10,12}).*?[号號]码[：:]\s*(\d{8})").unwrap(),
        // 模式3: 数电票（仅当无传统发票时）
        Regex::new(r"被(?:红冲|冲红)蓝字数电(?:票|发票)[号號]码[：:]\s*(\d{20})").unwrap(),
    )
});
static RED_NOTICE_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?:红字发票信息确认单|红冲通知单)\s*(?:编号|編號|号码|號碼|号|號)\s*[：:]\s*([0-9]{20})",
    )
    .unwrap()
});


static GOODS_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\*([^*]*)\*+(.*)$").unwrap());

fn extract_red_notice_no(clean: &str) -> Option<String> {
    RED_NOTICE_PATTERN
        .captures(clean)
        .map(|caps| caps[1].to_string())
}
fn normalize_remark(remark: &str) -> String {
    remark
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}
fn extract_rev_key_clean(clean: &str) -> Option<String> {
    let (re_trad_a, re_trad_b, re_elec) = &*INVOICE_PATTERNS;

    // 传统发票：先号码后代码
    if let Some(caps) = re_trad_a.captures(clean) {
        return Some(format!("{}-{}", &caps[2], &caps[1]));
    }

    // 对应正数发票
    if let Some(caps) = re_trad_b.captures(clean) {
        return Some(format!("{}-{}", &caps[1], &caps[2]));
    }

    // 数电发票
    if let Some(caps) = re_elec.captures(clean) {
        return Some(caps[1].to_string());
    }

    None
}



// pub fn data_to_opt_str(cell: &CellValue) -> Option<String> {
//     match cell {
//         CellValue::Empty => None,
//         CellValue::String(s) => {
//             let t = s.trim();
//             if t.is_empty() {
//                 None
//             } else {
//                 Some(t.to_string())
//             }
//         }
//         CellValue::Float(f) => {
//             if f.fract() == 0.0 && f.abs() < 1e15 {
//                 Some(format!("{:.0}", f))
//             } else {
//                 Some(f.to_string())
//             }
//         }
//         CellValue::Int(i) => Some(i.to_string()),
//         CellValue::Bool(b) => Some(b.to_string()),
//         CellValue::DateTime(dt) => Some(dt.to_string()),
//         CellValue::DateTimeIso(s) => Some(s.clone()),
//         CellValue::DurationIso(s) => Some(s.clone()),
//         CellValue::Error(e) => Some(format!("#ERR:{:?}", e)),
//     }
// }

/// Data → String（必填字段），为空返回 CellError::Empty
// pub fn data_to_str(cell: &CellValue) -> Result<String, CellError> {
//     data_to_opt_str(cell).ok_or(CellError::Empty)
// }

// /// Data → Option<f64>
// pub fn data_to_opt_f64(cell: &CellValue) -> Option<f64> {
//     match cell {
//         CellValue::Float(f) => Some(*f),
//         CellValue::Int(i) => Some(*i as f64),
//         CellValue::String(s) => s.trim().parse::<f64>().ok(),
//         _ => None,
//     }
// }

/// Data → f64（必填金额列），为空返回 CellError::InvalidNumber
// pub fn data_to_f64(cell: &CellValue) -> Result<f64, CellError> {
//     data_to_opt_f64(cell).ok_or(CellError::InvalidNumber)
// }

pub fn cell_to_f64(cell: &Data) -> Option<f64> {
    match cell {
        Data::Float(n) => Some(*n),
        Data::Int(n) => Some(*n as f64),
        Data::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

pub fn yuan_to_fen_str(s: &str) -> Result<i64, ()> {
    let n: f64 = s.trim().parse().map_err(|_| ())?;
    Ok((n * 100.0).round() as i64)
}

/// f64 元 → i64 分（四舍五入）
pub fn yuan_to_fen(yuan: f64) -> i64 {
    (yuan * 100.0).round() as i64
}
pub fn usize_to_u32(value: usize) -> u32 {
    value.try_into().expect("value exceeds u32::MAX")
}

/// 批量转换 usize 向量为 u32 向量
pub fn usize_vec_to_u32(vec: Vec<usize>) -> Vec<u32> {
    vec.into_iter().map(usize_to_u32).collect()
}

/// 根据发票代码、发票号码、数电发票号码生成 inv_unq_key
pub fn build_unq_key(code: Option<&str>, num: Option<&str>, e_num: Option<&str>) -> Option<String> {
    let code_ok = code.map(|s| !s.trim().is_empty()).unwrap_or(false);
    let num_ok = num.map(|s| !s.trim().is_empty()).unwrap_or(false);
    let e_num_ok = e_num
        .map(|s| {
            let t = s.trim();
            !t.is_empty() && t != "--"
        })
        .unwrap_or(false);

    if code_ok && num_ok {
        Some(format!("{}-{}", code.unwrap().trim(), num.unwrap().trim()))
    } else if e_num_ok {
        Some(e_num.unwrap().trim().to_string())
    } else {
        None
    }
}

pub fn parse_goods_name(raw: &str) -> (Option<String>, String) {
    let trimmed = raw.trim();
    //如果为空，则什么都不返回，几乎不存在。以防万一
    if trimmed.is_empty() {
        return (None, "".to_string());
    }

    // 正则匹配 *类别*+名称
    if let Some(caps) = GOODS_REGEX.captures(trimmed) {
        let category = caps
            .get(1)
            .map(|m| m.as_str().trim())
            .unwrap_or("")
            .to_string();

        let name = caps
            .get(2)
            .map(|m| m.as_str().trim().to_string())
            .unwrap_or_default();

        return (Some(category), name);
    }

    // 特殊情况：详见销货清单
    (None, trimmed.to_string())
}

// pub fn extract_rev_key(remark: &str) -> Option<String> {
//     let clean = remark.replace(['\r', '\n'], " ").trim().to_string();

//     if clean.is_empty() {
//         return None;
//     }
//     let (re_trad_a, re_trad_b, re_elec) = &*INVOICE_PATTERNS;

//     // 优先级1: 传统发票（先号码后代码）- 混合场景优先匹配此模式
//     if let Some(caps) = re_trad_a.captures(&clean) {
//         return Some(format!("{}-{}", &caps[2], &caps[1])); // 代码-号码
//     }

//     // 优先级2: 传统发票（先代码后号码）
//     if let Some(caps) = re_trad_b.captures(&clean) {
//         return Some(format!("{}-{}", &caps[1], &caps[2])); // 代码-号码
//     }

//     // 优先级3: 数电票
//     if let Some(caps) = re_elec.captures(&clean) {
//         return Some(caps[1].to_string()); // 直接返回20位号码
//     }

//     // 未匹配 → 返回空字符串（业务层需校验）
//     None
// }

pub fn extract_rev_key(remark: &str) -> Option<String> {
    let clean = normalize_remark(remark);

    if clean.is_empty() {
        return None;
    }

    extract_rev_key_clean(&clean)
}

pub fn extract_red_invoice_reference(
    remark: &str,
) -> Option<RedInvoiceReference> {
    let clean = normalize_remark(remark);

    if clean.is_empty() {
        return None;
    }

    let blue_invoice_key = extract_rev_key_clean(&clean)?;
    let red_notice_no = extract_red_notice_no(&clean);

    Some(RedInvoiceReference {
        blue_invoice_key,
        red_notice_no,
    })
}

pub fn is_empty_row(row: &SheetRow) -> bool {
    row.iter().all(CellValue::is_empty_val)
}
pub fn c_is_empty_row(row: &[Data]) -> bool {
    row.iter().all(|cell| match cell {
        Data::Empty => true,
        Data::String(s) => s.trim().is_empty(),
        _ => false,
    })
}

// pub fn is_total_row(row: &SheetRow) -> bool {
//     row.get(0)
//         .and_then(CellValue::as_str)
//         .map(|s| s.contains("合计"))
//         .unwrap_or(false)
// }
pub fn is_total_row(row: &SheetRow, shared: &[String]) -> bool {
    row.get(0)
        .and_then(|v| v.as_str(shared))
        .map(|s| s.contains("合计"))
        .unwrap_or(false)
}

#[inline]
pub fn cell<'a>(row: &'a [CellValue], idx: usize) -> Option<&'a CellValue> {
    row.get(idx)
}

#[inline]
pub fn normalize_number_string(n: f64) -> String {
    if n.is_nan() || n.is_infinite() {
        // 非法数值，保留原样的字符串表示
        return n.to_string();
    }

    // 如果是整数，直接返回整数字符串
    if n.fract().abs() < std::f64::EPSILON {
        return format!("{}", n.trunc() as i64);
    }

    // 否则格式化为字符串并去掉尾随的零
    let mut s = format!("{}", n);
    if let Some(dot_pos) = s.find('.') {
        // 去掉末尾的 0
        let mut end = s.len();
        while end > dot_pos + 1 && s.as_bytes()[end - 1] == b'0' {
            end -= 1;
        }
        // 如果小数点后面只有一个数字且为 0，去掉小数点
        if end > dot_pos + 1 && s.as_bytes()[end - 1] == b'.' {
            end -= 1;
        }
        s.truncate(end);
    }
    s
}


// #[inline]
// pub fn req(row: &[CellValue], idx: usize) -> Result<String, (usize, ())> {
//     match cell(row, idx) {
//         Some(CellValue::Str(s)) if !s.trim().is_empty() => Ok(s.trim().to_string()),
//         Some(CellValue::Num(n)) => Ok(normalize_number_string(*n)),
//         _ => Err((idx, ())),
//     }
// }
#[inline]
pub fn req_str(row: &[CellValue], idx: usize, shared: &[String]) -> Result<String, (usize, ())> {
    match cell(row, idx) {
        Some(v) => {
            if let Some(s) = v.as_str(shared) {
                let s = s.trim();
                if !s.is_empty() {
                    return Ok(s.to_string());
                }
            }

            if let Some(n) = v.as_f64(shared) {
                return Ok(normalize_number_string(n));
            }

            Err((idx, ()))
        }
        None => Err((idx, ())),
    }
}
// #[inline]
// pub fn opt(row: &[CellValue], idx: usize) -> Option<String> {
//     match cell(row, idx) {
//         Some(CellValue::Str(s)) if !s.trim().is_empty() => Some(s.trim().to_string()),
//         Some(CellValue::Num(n)) => Some(normalize_number_string(*n)),
//         _ => None,
//     }
// }
#[inline]
pub fn opt_str(row: &[CellValue], idx: usize, shared: &[String]) -> Option<String> {
    match cell(row, idx) {
        Some(v) => {
            if let Some(s) = v.as_str(shared) {
                let s = s.trim();
                if !s.is_empty() {
                    return Some(s.to_string());
                }
            }

            if let Some(n) = v.as_f64(shared) {
                return Some(normalize_number_string(n));
            }

            None
        }
        None => None,
    }
}

// #[inline]
// pub fn req_amount(row: &[CellValue], idx: usize) -> Result<String, (usize, ())> {
//     match cell(row, idx) {
//         Some(CellValue::Str(s)) if !s.trim().is_empty() => Ok(s.trim().to_string()),
//         Some(CellValue::Num(n)) => Ok(normalize_number_string(*n)),
//         _ => Err((idx, ())),
//     }
// }
#[inline]
pub fn req_amount_str(
    row: &[CellValue],
    idx: usize,
    shared: &[String],
) -> Result<String, (usize, ())> {
    match cell(row, idx) {
        Some(v) => {
            if let Some(s) = v.as_str(shared) {
                let s = s.trim();
                if !s.is_empty() {
                    return Ok(s.to_string());
                }
            }

            if let Some(n) = v.as_f64(shared) {
                return Ok(normalize_number_string(n));
            }

            Err((idx, ()))
        }
        None => Err((idx, ())),
    }
}

// #[inline]
// pub fn opt_f64(row: &[CellValue], idx: usize) -> Option<f64> {
//     match cell(row, idx) {
//         Some(CellValue::Num(n)) => Some(*n),
//         Some(CellValue::Str(s)) => s.trim().parse::<f64>().ok(),
//         _ => None,
//     }
// }
#[inline]
pub fn opt_f64(row: &[CellValue], idx: usize, shared: &[String]) -> Option<f64> {
    match cell(row, idx) {
      
        Some(CellValue::InlineStr(s)) => s.trim().parse().ok(),
        Some(CellValue::StrRef(i)) => shared.get(*i)?.trim().parse().ok(), // ← 关键修复
        _ => None,
    }
}

#[inline]
pub fn get_str<'a>(row: &'a [CellValue], idx: usize, shared: &'a [String]) -> Option<&'a str> {
    row.get(idx).and_then(|v| v.as_str(shared))
}

pub fn total_row_check(row: &[Data]) -> bool {
    if let Some(Data::String(s)) = row.get(0) {
        s.trim().contains("合计")
    } else {
        false
    }
}

/// 将 Excel 中的日期数据（DateTime, String, Float 等）统一转换为 "YYYY-MM-DD" 格式的字符串。
///
/// 返回 Ok("2025-03-25") 或 Err("错误原因字符串")。
pub fn excel_date_to_string(cell: Option<&Data>) -> Result<String, String> {
    let cell = match cell {
        Some(c) if !matches!(c, Data::Empty | Data::Error(_)) => c,
        _ => return Err("单元格为空或存在错误".to_string()),
    };

    match cell {
        // 情况 A：calamine 自动识别为了 DateTime 类型
        Data::DateTime(dt) => match dt.as_datetime() {
            Some(naive_dt) => Ok(naive_dt.format("%Y-%m-%d").to_string()),
            None => Err("Excel日期转换失败：超出了有效日期范围".to_string()),
        },

        // 情况 B：被识别为了字符串，比如 "2025/3/25" 或 "2025-03-25"
        Data::String(s) => {
            let s = s.trim();
            let formats = ["%Y/%m/%d", "%Y-%m-%d", "%Y年%m月%d日"];

            for fmt in formats {
                if let Ok(dt) = NaiveDate::parse_from_str(s, fmt) {
                    return Ok(dt.format("%Y-%m-%d").to_string());
                }
            }

            Err(format!(
                "日期字符串格式无法识别：「{}」，支持如 2025/3/25 或 2025-03-25",
                s
            ))
        }

        // 情况 C：兜底处理，Excel 把日期存成了数字序列号 (如 45743.0)
        Data::Float(f) => {
            if *f > 1.0 && *f < 2958465.0 {
                let base_date = NaiveDate::from_ymd_opt(1899, 12, 30).unwrap();
                let days = f.trunc() as i64;
                match base_date.checked_add_signed(Duration::days(days)) {
                    Some(final_date) => Ok(final_date.format("%Y-%m-%d").to_string()),
                    None => Err("数字序列号转换日期失败".to_string()),
                }
            } else {
                Err(format!("无法将数字 {} 识别为有效日期", f))
            }
        }

        // 情况 D：同情况 C，针对纯整数
        Data::Int(i) => {
            if *i > 1 && *i < 2958465 {
                let base_date = NaiveDate::from_ymd_opt(1899, 12, 30).unwrap();
                match base_date.checked_add_signed(Duration::days(*i)) {
                    Some(final_date) => Ok(final_date.format("%Y-%m-%d").to_string()),
                    None => Err("整数序列号转换日期失败".to_string()),
                }
            } else {
                Err(format!("无法将整数 {} 识别为有效日期", i))
            }
        }

        // 其他不可能出现的类型 (Bool 等)
        _ => Err("数据类型异常，无法解析为日期".to_string()),
    }
}

/// calamine::Data -> Option<String>
///
/// 注意：
/// 如果 Excel 把 20 位数电票号码当数字保存，Excel 本身可能已经丢失精度。
/// 所以建议源 Excel 中发票号码类字段保持文本格式。
pub fn cell_to_opt_string(cell: Option<&Data>) -> Option<String> {
    let cell = cell?;

    let value = match cell {
        Data::Empty => return None,

        Data::String(s) => s.trim().to_string(),

        Data::Float(n) => normalize_number_string(*n),

        Data::Int(n) => n.to_string(),

        Data::Bool(b) => b.to_string(),

        Data::DateTime(dt) => dt.to_string(),

        Data::DateTimeIso(s) => s.trim().to_string(),

        Data::DurationIso(s) => s.trim().to_string(),

        Data::Error(_) => return None,
    };

    let value = value.trim().to_string();

    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

pub fn cell_to_string(cell: &Data) -> Option<String> {
    let s = match cell {
        Data::Empty => return None,
        Data::String(s) => s.trim().to_string(),
        Data::Float(n) => normalize_number_string(*n),
        Data::Int(n) => n.to_string(),
        Data::Bool(b) => b.to_string(),
        Data::DateTime(dt) => dt.to_string(),
        Data::DateTimeIso(s) => s.trim().to_string(),
        Data::DurationIso(s) => s.trim().to_string(),
        Data::Error(_) => return None,
    };
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}
