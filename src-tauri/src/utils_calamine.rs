// src-tauri/src/utils.rs
use crate::error::CellError;
use calamine::Data;

use regex::Regex;
use std::sync::LazyLock;

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

static GOODS_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\*([^*]*)\*+(.*)$").unwrap());

#[inline]
pub fn opt_str(row: &[Data], idx: usize) -> Option<String> {
    row.get(idx).and_then(data_to_opt_str)
}

#[inline]
pub fn req_str(row: &[Data], idx: usize) -> Result<String, (usize, ())> {
    opt_str(row, idx).ok_or((idx, ()))
}

pub fn data_to_opt_str(cell: &Data) -> Option<String> {
    match cell {
        Data::Empty => None,
        Data::String(s) => {
            let t = s.trim();
            if t.is_empty() {
                None
            } else {
                Some(t.to_string())
            }
        }
        Data::Float(f) => {
            if f.fract() == 0.0 && f.abs() < 1e15 {
                Some(format!("{:.0}", f))
            } else {
                Some(f.to_string())
            }
        }
        Data::Int(i) => Some(i.to_string()),
        Data::Bool(b) => Some(b.to_string()),
        Data::DateTime(dt) => Some(dt.to_string()),
        Data::DateTimeIso(s) => Some(s.clone()),
        Data::DurationIso(s) => Some(s.clone()),
        Data::Error(e) => Some(format!("#ERR:{:?}", e)),
    }
}

/// Data → String（必填字段），为空返回 CellError::Empty
pub fn data_to_str(cell: &Data) -> Result<String, CellError> {
    data_to_opt_str(cell).ok_or(CellError::Empty)
}

/// Data → Option<f64>
pub fn data_to_opt_f64(cell: &Data) -> Option<f64> {
    match cell {
        Data::Float(f) => Some(*f),
        Data::Int(i) => Some(*i as f64),
        Data::String(s) => s.trim().parse::<f64>().ok(),
        _ => None,
    }
}

/// Data → f64（必填金额列），为空返回 CellError::InvalidNumber
///
#[inline]
pub fn opt_f64(row: &[Data], idx: usize) -> Option<f64> {
    row.get(idx).and_then(data_to_opt_f64)
}

pub fn data_to_f64(cell: &Data) -> Result<f64, CellError> {
    data_to_opt_f64(cell).ok_or(CellError::InvalidNumber)
}
#[inline]
pub fn req_f64(row: &[Data], idx: usize) -> Result<f64, (usize, ())> {
    opt_f64(row, idx).ok_or((idx, ()))
}
/// f64 元 → i64 分（四舍五入）
pub fn yuan_to_fen(yuan: f64) -> i64 {
    (yuan * 100.0).round() as i64
}

pub fn yuan_to_fen_str(s: &str) -> i64 {
    let n: f64 = s.trim().parse().unwrap_or(0.0);
    (n * 100.0).round() as i64
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

pub fn extract_rev_key(remark: &str) -> Option<String> {
    let clean = remark.replace(['\r', '\n'], " ").trim().to_string();

    if clean.is_empty() {
        return None;
    }
    let (re_trad_a, re_trad_b, re_elec) = &*INVOICE_PATTERNS;

    // 优先级1: 传统发票（先号码后代码）- 混合场景优先匹配此模式
    if let Some(caps) = re_trad_a.captures(&clean) {
        return Some(format!("{}-{}", &caps[2], &caps[1])); // 代码-号码
    }

    // 优先级2: 传统发票（先代码后号码）
    if let Some(caps) = re_trad_b.captures(&clean) {
        return Some(format!("{}-{}", &caps[1], &caps[2])); // 代码-号码
    }

    // 优先级3: 数电票
    if let Some(caps) = re_elec.captures(&clean) {
        return Some(caps[1].to_string()); // 直接返回20位号码
    }

    // 未匹配 → 返回空字符串（业务层需校验）
    None
}
