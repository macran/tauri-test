use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use specta::Type;
use strum::EnumString;

use crate::DbState;

const INVOICE_STATE_NORMAL: i64 = 1;
const INVOICE_STATE_REVERSED_FULL: i64 = 3;
const INVOICE_STATE_REVERSED_PARTIAL: i64 = 4;

const MATCH_STATUS_NORMAL: i64 = 1;
const MATCH_STATUS_EXTERNAL_HISTORY: i64 = 2;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type, EnumString)]
#[serde(rename_all = "camelCase")]
pub enum MatchResultType {
    #[strum(serialize = "成功")]
    Ok = 1,
    #[strum(serialize = "无蓝字发票")]
    BlueMissing = 2,
    #[strum(serialize = "红字发票异常")]
    Abnormal = 3,
}

impl MatchResultType {
    fn to_db_value(self) -> u8 {
        self as u8
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "成功",
            Self::BlueMissing => "无蓝字发票",
            Self::Abnormal => "红字发票异常",
        }
    }
}

impl fmt::Display for MatchResultType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str((*self).as_str())
    }
}

#[derive(Debug, Clone)]
struct RedHeader {
    id: i64,
    inv_unq_key: String,
    rev_org_key: String,
}

#[derive(Debug, Clone)]
struct RedDetail {
    id: i64,
    line_no: u32,
    goods_name: String,
    spec_model: Option<String>,
    unit_price: Option<String>,
    quantity: Option<f64>,
    amount: i64,
    tax: i64,
    total: i64,
}

#[derive(Debug, Clone)]
struct BlueHeader {
    id: i64,
    inv_unq_key: String,
    state: i64,
    total: i64,
    issue_date: String,
    seller_tax_no: String,
    seller_name: String,
}

#[derive(Debug, Clone)]
struct BlueUnit {
    id: i64,
    line_no: u32,
    goods_name: String,
    spec_model: Option<String>,
    unit_price: Option<String>,
    quantity: Option<f64>,
    amount: i64,
    #[allow(dead_code)]
    tax: i64,
    #[allow(dead_code)]
    total: i64,
    remaining: i64,
    remaining_amount: i64,
    remaining_tax: i64,
    discount_detail_id: Option<i64>,
}

#[derive(Debug, Clone)]
struct MatchRecord {
    red_header_id: i64,
    red_detail_id: i64,
    blue_header_id: Option<i64>,
    blue_detail_id: Option<i64>,
    blue_discount_detail_id: Option<i64>,
    matched_quantity: Option<f64>,
    matched_amount: i64,
    matched_tax: i64,
    matched_total: i64,
    match_result_type: MatchResultType,
    match_status: i64,
    match_note: Option<String>,
    match_batch_no: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ReversalAbnormalItem {
    pub red_inv_unq_key: String,
    pub red_line_no: u32,
    pub match_result_type: MatchResultType,
    pub match_note: Option<String>,
    pub blue_inv_unq_key: Option<String>,
    pub blue_issue_date: Option<String>,
    pub blue_seller_tax_no: Option<String>,
    pub blue_seller_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ReversalMatchSummary {
    pub match_run_no: i32,
    pub ok_count: u32,
    pub blue_missing_count: u32,
    pub abnormal_count: u32,
    pub abnormal_items: Vec<ReversalAbnormalItem>,
}

#[tauri::command]
#[specta::specta]
pub async fn match_reversal_invoices(
    start_date: String,
    end_date: String,
    db: tauri::State<'_, DbState>,
) -> Result<ReversalMatchSummary, String> {
    let conn_arc = Arc::clone(&db.conn);

    tokio::task::spawn_blocking(move || {
        let mut conn = conn_arc.lock().unwrap();
        run_match(&mut conn, &start_date, &end_date)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e: rusqlite::Error| e.to_string())
}

/// 按红字发票开票日期区间（[start_date, end_date]，闭区间，格式 YYYY-MM-DD）扫描待匹配的红字发票。
/// 不再依赖导入批次号：同一红字明细一旦已经产生过“正常状态”的匹配结果（match_status = NORMAL）
/// 就不会被重复处理；如果之前因为找不到蓝字发票而只留下了“无蓝字发票”占位记录
/// （match_status = EXTERNAL_HISTORY），只要用户后续把缺失的蓝字发票补录进数据库，
/// 再次选择覆盖该红票开票日期的区间重新核对，就会自动清理旧占位记录并重新尝试匹配——
/// 不需要为“补录后重新匹配”单独写一套流程。
fn run_match(
    conn: &mut Connection,
    start_date: &str,
    end_date: &str,
) -> Result<ReversalMatchSummary, rusqlite::Error> {
    let match_run_no = next_match_run_no(conn)?;

    let red_headers = query_red_headers(conn, start_date, end_date)?;
    let mut all_records = Vec::<MatchRecord>::new();
    let mut abnormal_items = Vec::<ReversalAbnormalItem>::new();
    let mut blue_missing_count = 0_u32;
    let mut abnormal_count = 0_u32;
    let mut blue_headers_to_update = HashMap::<i64, BlueHeader>::new();
    // 同一次运行内，多张红票可能红冲同一张蓝票：这两个缓存让后处理的红票能看到
    // 前面红票刚刚（尚未提交到数据库的）占用的剩余额度，避免超额匹配/重复计算。
    let mut blue_units_cache = HashMap::<i64, Vec<BlueUnit>>::new();
    let mut blue_matched_total_cache = HashMap::<i64, i64>::new();
    let mut red_detail_ids_processed = Vec::<i64>::new();

    for red_hdr in &red_headers {
        let red_details = query_red_details(conn, red_hdr.id)?;
        if red_details.is_empty() {
            continue;
        }
        red_detail_ids_processed.extend(red_details.iter().map(|d| d.id));

        let blue_hdr = match query_blue_header(conn, &red_hdr.rev_org_key)? {
            Some(header) => header,
            None => {
                for rd in &red_details {
                    let note = format!(
                        "找不到数据库内的蓝字发票：{}；该红字明细按数据库外历史数据处理，后续不再自动匹配",
                        red_hdr.rev_org_key
                    );

                    all_records.push(MatchRecord {
                        red_header_id: red_hdr.id,
                        red_detail_id: rd.id,
                        blue_header_id: None,
                        blue_detail_id: None,
                        blue_discount_detail_id: None,
                        matched_quantity: None,
                        matched_amount: 0,
                        matched_tax: 0,
                        matched_total: 0,
                        match_result_type: MatchResultType::BlueMissing,
                        match_status: MATCH_STATUS_EXTERNAL_HISTORY,
                        match_note: Some(note.clone()),
                        match_batch_no: match_run_no,
                    });

                    blue_missing_count += 1;
                    abnormal_items.push(ReversalAbnormalItem {
                        red_inv_unq_key: red_hdr.inv_unq_key.clone(),
                        red_line_no: rd.line_no,
                        match_result_type: MatchResultType::BlueMissing,
                        match_note: Some(note),
                        blue_inv_unq_key: None,
                        blue_issue_date: None,
                        blue_seller_tax_no: None,
                        blue_seller_name: None,
                    });
                }
                continue;
            }
        };

        if !blue_units_cache.contains_key(&blue_hdr.id) {
            let units = query_blue_units_with_remaining(conn, blue_hdr.id)?;
            blue_units_cache.insert(blue_hdr.id, units);
        }
        let blue_units = blue_units_cache.get_mut(&blue_hdr.id).unwrap();

        let mut records = match_normal(
            red_hdr,
            &red_details,
            &blue_hdr,
            blue_units,
            match_run_no,
        );

        let existing_matched_total = match blue_matched_total_cache.get(&blue_hdr.id) {
            Some(total) => *total,
            None => {
                let total = query_existing_blue_matched_total(conn, blue_hdr.id)?;
                blue_matched_total_cache.insert(blue_hdr.id, total);
                total
            }
        };
        let new_matched_total: i64 = records
            .iter()
            .filter(|record| {
                record.blue_header_id == Some(blue_hdr.id)
                    && record.blue_detail_id.is_some()
                    && record.match_status == MATCH_STATUS_NORMAL
            })
            .map(|record| record.matched_total)
            .sum();
        // 本次红票占用的额度立刻计入缓存，供同一次运行里后面还会命中同一张蓝票的红票使用。
        *blue_matched_total_cache.get_mut(&blue_hdr.id).unwrap() += new_matched_total;

        let net_remaining = blue_hdr.total - existing_matched_total - new_matched_total;
        let full_state_note = format!(
            "蓝票状态为已红冲-全额，但按历史+本次匹配后仍剩 {} 分",
            net_remaining
        );

        if blue_hdr.state == INVOICE_STATE_REVERSED_FULL && net_remaining.abs() > 1 {
            let related_detail_ids: Vec<i64> = records
                .iter()
                .filter(|record| {
                    record.blue_header_id == Some(blue_hdr.id)
                        && record.blue_detail_id.is_some()
                        && record.match_status == MATCH_STATUS_NORMAL
                })
                .map(|record| record.red_detail_id)
                .collect();

            for record in &mut records {
                if record.blue_header_id == Some(blue_hdr.id)
                    && record.blue_detail_id.is_some()
                    && record.match_status == MATCH_STATUS_NORMAL
                {
                    record.match_result_type = MatchResultType::Abnormal;
                    record.match_note = Some(append_note(
                        record.match_note.as_deref(),
                        &full_state_note,
                    ));
                }
            }

            for red_detail_id in related_detail_ids {
                if let Some(rd) = red_details.iter().find(|detail| detail.id == red_detail_id) {
                    abnormal_count += 1;
                    abnormal_items.push(ReversalAbnormalItem {
                        red_inv_unq_key: red_hdr.inv_unq_key.clone(),
                        red_line_no: rd.line_no,
                        match_result_type: MatchResultType::Abnormal,
                        match_note: Some(full_state_note.clone()),
                        blue_inv_unq_key: Some(blue_hdr.inv_unq_key.clone()),
                        blue_issue_date: Some(blue_hdr.issue_date.clone()),
                        blue_seller_tax_no: Some(blue_hdr.seller_tax_no.clone()),
                        blue_seller_name: Some(blue_hdr.seller_name.clone()),
                    });
                }
            }
        } else {
            for record in &records {
                if record.match_result_type != MatchResultType::Abnormal {
                    continue;
                }

                abnormal_count += 1;
                abnormal_items.push(ReversalAbnormalItem {
                    red_inv_unq_key: red_hdr.inv_unq_key.clone(),
                    red_line_no: red_line_no_by_detail_id(&red_details, record.red_detail_id),
                    match_result_type: MatchResultType::Abnormal,
                    match_note: record.match_note.clone(),
                    blue_inv_unq_key: Some(blue_hdr.inv_unq_key.clone()),
                    blue_issue_date: Some(blue_hdr.issue_date.clone()),
                    blue_seller_tax_no: Some(blue_hdr.seller_tax_no.clone()),
                    blue_seller_name: Some(blue_hdr.seller_name.clone()),
                });
            }
        }

        let has_real_match = records.iter().any(|record| {
            record.blue_header_id == Some(blue_hdr.id)
                && record.blue_detail_id.is_some()
                && record.match_status == MATCH_STATUS_NORMAL
        });

        if has_real_match {
            blue_headers_to_update
                .entry(blue_hdr.id)
                .or_insert_with(|| blue_hdr.clone());
        }

        all_records.extend(records);
    }

    let tx = conn.transaction()?;
    // 本次(重新)处理过的红字明细，先清掉它们身上遗留的"无蓝字发票"占位记录，
    // 避免旧占位记录和这次的新结果同时留在表里。
    delete_stale_external_history(&tx, &red_detail_ids_processed)?;
    let ok_count = insert_match_records(&tx, &all_records)?;
    update_blue_states(&tx, &blue_headers_to_update)?;
    tx.commit()?;

    Ok(ReversalMatchSummary {
        match_run_no,
        ok_count,
        blue_missing_count,
        abnormal_count,
        abnormal_items,
    })
}

fn match_normal(
    red_hdr: &RedHeader,
    red_details: &[RedDetail],
    blue_hdr: &BlueHeader,
    blue_units: &mut Vec<BlueUnit>,
    batch_no: i32,
) -> Vec<MatchRecord> {
    let mut records = Vec::new();
    let mut red_groups: HashMap<String, Vec<&RedDetail>> = HashMap::new();

    for detail in red_details {
        red_groups
            .entry(item_key(&detail.goods_name, &detail.spec_model))
            .or_default()
            .push(detail);
    }

    for (key, mut red_group) in red_groups {
        let mut blue_group: Vec<&mut BlueUnit> = blue_units
            .iter_mut()
            .filter(|unit| {
                unit.amount > 0
                    && unit.remaining > 0
                    && item_key(&unit.goods_name, &unit.spec_model) == key
            })
            .collect();

        red_group.sort_by_key(|detail| detail.line_no);
        blue_group.sort_by_key(|unit| unit.line_no);

        if red_group.len() == 1 {
            match_single_red_detail(
                red_hdr,
                red_group[0],
                blue_hdr,
                &mut blue_group,
                batch_no,
                &mut records,
            );
        } else {
            for detail in red_group {
                match_multi_red_detail(
                    red_hdr,
                    detail,
                    blue_hdr,
                    &mut blue_group,
                    batch_no,
                    &mut records,
                );
            }
        }
    }

    for rd in red_details {
        let matched_total: i64 = records
            .iter()
            .filter(|record| record.red_detail_id == rd.id)
            .map(|record| record.matched_total)
            .sum();
        let need = rd.total.abs();

        if matched_total < need {
            let left = need - matched_total;
            let (left_amount, left_tax) = split_amount_tax(rd, left);

            records.push(MatchRecord {
                red_header_id: red_hdr.id,
                red_detail_id: rd.id,
                blue_header_id: Some(blue_hdr.id),
                blue_detail_id: None,
                blue_discount_detail_id: None,
                matched_quantity: unmatched_quantity(rd.quantity, matched_total, need),
                matched_amount: left_amount,
                matched_tax: left_tax,
                matched_total: left,
                match_result_type: MatchResultType::Abnormal,
                match_status: MATCH_STATUS_NORMAL,
                match_note: Some(format!(
                    "蓝字发票存在，但同名同规格可用剩余金额不足；红字需匹配 {} 分，已匹配 {} 分，剩余 {} 分未匹配",
                    need, matched_total, left
                )),
                match_batch_no: batch_no,
            });
        }
    }

    records
}

fn match_single_red_detail(
    red_hdr: &RedHeader,
    rd: &RedDetail,
    blue_hdr: &BlueHeader,
    blue_group: &mut Vec<&mut BlueUnit>,
    batch_no: i32,
    records: &mut Vec<MatchRecord>,
) {
    let need = rd.total.abs();
    let mut candidates: Vec<(usize, Option<f64>, i64, u32)> = blue_group
        .iter()
        .enumerate()
        .filter(|(_, unit)| unit.remaining >= need)
        .map(|(index, unit)| {
            (index, price_distance(rd, unit), unit.remaining, unit.line_no)
        })
        .collect();

    candidates.sort_by(|a, b| compare_candidates(a, b));

    if let Some((index, _, _, _)) = candidates.first().copied() {
        let unit = &mut blue_group[index];
        let take_total = need;
        let (take_amount, take_tax) = split_amount_tax(rd, take_total);
        let quantity = matched_quantity(rd.quantity, unit.quantity, take_total, need);

        unit.remaining -= take_total;
        unit.remaining_amount -= take_amount;
        unit.remaining_tax -= take_tax;

        let note = match price_distance(rd, unit) {
            Some(distance) if distance > 0.0001 => {
                Some(format!("按同名同规格匹配，单价接近优先，价差={:.8}", distance))
            }
            Some(_) => None,
            None => Some("按同名同规格匹配；该蓝票无数量/无单价，不参考单价".to_string()),
        };

        records.push(MatchRecord {
            red_header_id: red_hdr.id,
            red_detail_id: rd.id,
            blue_header_id: Some(blue_hdr.id),
            blue_detail_id: Some(unit.id),
            blue_discount_detail_id: unit.discount_detail_id,
            matched_quantity: quantity,
            matched_amount: take_amount,
            matched_tax: take_tax,
            matched_total: take_total,
            match_result_type: MatchResultType::Ok,
            match_status: MATCH_STATUS_NORMAL,
            match_note: note,
            match_batch_no: batch_no,
        });
        return;
    }

    blue_group.sort_by(|a, b| compare_optional_price_and_line(price_distance(rd, a), a.line_no, price_distance(rd, b), b.line_no));

    let mut left = need;
    for unit in blue_group.iter_mut() {
        if left == 0 {
            break;
        }
        if unit.remaining <= 0 {
            continue;
        }

        let take_total = unit.remaining.min(left);
        let (take_amount, take_tax) = split_amount_tax(rd, take_total);
        let quantity = matched_quantity(rd.quantity, unit.quantity, take_total, need);

        unit.remaining -= take_total;
        unit.remaining_amount -= take_amount;
        unit.remaining_tax -= take_tax;
        left -= take_total;

        let note = match price_distance(rd, unit) {
            Some(distance) if distance > 0.0001 => {
                Some(format!("跨行凑匹配，单价接近优先，价差={:.8}", distance))
            }
            Some(_) => Some("跨行凑匹配".to_string()),
            None => Some("跨行凑匹配；该蓝票无数量/无单价，不参考单价".to_string()),
        };

        records.push(MatchRecord {
            red_header_id: red_hdr.id,
            red_detail_id: rd.id,
            blue_header_id: Some(blue_hdr.id),
            blue_detail_id: Some(unit.id),
            blue_discount_detail_id: unit.discount_detail_id,
            matched_quantity: quantity,
            matched_amount: take_amount,
            matched_tax: take_tax,
            matched_total: take_total,
            match_result_type: MatchResultType::Ok,
            match_status: MATCH_STATUS_NORMAL,
            match_note: note,
            match_batch_no: batch_no,
        });
    }
}

fn match_multi_red_detail(
    red_hdr: &RedHeader,
    rd: &RedDetail,
    blue_hdr: &BlueHeader,
    blue_group: &mut Vec<&mut BlueUnit>,
    batch_no: i32,
    records: &mut Vec<MatchRecord>,
) {
    let need = rd.total.abs();
    let mut candidates: Vec<(usize, Option<f64>, i64, u32)> = blue_group
        .iter()
        .enumerate()
        .filter(|(_, unit)| unit.remaining > 0)
        .map(|(index, unit)| {
            (
                index,
                price_distance(rd, unit),
                if unit.remaining >= need {
                    unit.remaining - need
                } else {
                    i64::MAX / 4
                },
                unit.line_no,
            )
        })
        .collect();

    candidates.sort_by(|a, b| compare_candidates(a, b));

    if let Some((index, _, _, _)) = candidates.first().copied() {
        let unit = &mut blue_group[index];
        let take_total = need.min(unit.remaining);
        let (take_amount, take_tax) = split_amount_tax(rd, take_total);
        let quantity = matched_quantity(rd.quantity, unit.quantity, take_total, need);

        unit.remaining -= take_total;
        unit.remaining_amount -= take_amount;
        unit.remaining_tax -= take_tax;

        let note = match price_distance(rd, unit) {
            Some(distance) if distance > 0.0001 => {
                Some(format!("多行匹配，单价接近优先，价差={:.8}", distance))
            }
            Some(_) => None,
            None => Some("多行匹配；该蓝票无数量/无单价，不参考单价".to_string()),
        };

        records.push(MatchRecord {
            red_header_id: red_hdr.id,
            red_detail_id: rd.id,
            blue_header_id: Some(blue_hdr.id),
            blue_detail_id: Some(unit.id),
            blue_discount_detail_id: unit.discount_detail_id,
            matched_quantity: quantity,
            matched_amount: take_amount,
            matched_tax: take_tax,
            matched_total: take_total,
            match_result_type: MatchResultType::Ok,
            match_status: MATCH_STATUS_NORMAL,
            match_note: note,
            match_batch_no: batch_no,
        });
    }
}

fn compare_candidates(
    a: &(usize, Option<f64>, i64, u32),
    b: &(usize, Option<f64>, i64, u32),
) -> std::cmp::Ordering {
    match (&a.1, &b.1) {
        (Some(x), Some(y)) => x
            .partial_cmp(y)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.2.cmp(&b.2))
            .then_with(|| a.3.cmp(&b.3)),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => a.2.cmp(&b.2).then_with(|| a.3.cmp(&b.3)),
    }
}

fn compare_optional_price_and_line(
    a_price: Option<f64>,
    a_line: u32,
    b_price: Option<f64>,
    b_line: u32,
) -> std::cmp::Ordering {
    match (a_price, b_price) {
        (Some(a), Some(b)) => a
            .partial_cmp(&b)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a_line.cmp(&b_line)),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => a_line.cmp(&b_line),
    }
}

fn split_amount_tax(rd: &RedDetail, take_total: i64) -> (i64, i64) {
    let original_total = rd.total.abs();
    let original_tax = rd.tax.abs();

    if original_total <= 0 || take_total <= 0 {
        return (0, 0);
    }

    if take_total >= original_total {
        return (rd.amount.abs(), original_tax);
    }

    let mut take_tax =
        (original_tax as f64 * take_total as f64 / original_total as f64).round() as i64;
    take_tax = take_tax.clamp(0, take_total);
    (take_total - take_tax, take_tax)
}

fn matched_quantity(
    red_quantity: Option<f64>,
    _blue_quantity: Option<f64>,
    take_total: i64,
    need_total: i64,
) -> Option<f64> {
    match red_quantity {
        Some(quantity) if need_total > 0 => {
            Some(quantity.abs() * take_total as f64 / need_total as f64)
        }
        _ => None,
    }
}

fn unmatched_quantity(
    red_quantity: Option<f64>,
    matched_total: i64,
    need_total: i64,
) -> Option<f64> {
    match red_quantity {
        Some(quantity) if need_total > 0 && matched_total < need_total => {
            Some(quantity.abs() * (need_total - matched_total) as f64 / need_total as f64)
        }
        _ => None,
    }
}

fn item_key(goods_name: &str, spec_model: &Option<String>) -> String {
    format!(
        "{}|{}",
        goods_name.trim(),
        spec_model.as_deref().unwrap_or("").trim()
    )
}

fn normalize_unit_price(net_amount: i64, quantity: Option<f64>) -> Option<String> {
    match quantity {
        Some(quantity) if quantity.abs() > 1e-9 => {
            Some(format!("{:.8}", net_amount as f64 / quantity))
        }
        _ => None,
    }
}

fn parse_price(price: &Option<String>) -> Option<f64> {
    price.as_ref()?.trim().parse::<f64>().ok()
}

fn price_distance(red_detail: &RedDetail, blue_unit: &BlueUnit) -> Option<f64> {
    Some((parse_price(&red_detail.unit_price)? - parse_price(&blue_unit.unit_price)?).abs())
}

fn append_note(old: Option<&str>, suffix: &str) -> String {
    match old {
        Some(value) if !value.trim().is_empty() => format!("{}；{}", value, suffix),
        _ => suffix.to_string(),
    }
}

fn red_line_no_by_detail_id(red_details: &[RedDetail], red_detail_id: i64) -> u32 {
    red_details
        .iter()
        .find(|detail| detail.id == red_detail_id)
        .map(|detail| detail.line_no)
        .unwrap_or(0)
}

fn is_discount_pair(
    a: &(i64, u32, String, Option<String>, Option<String>, Option<f64>, i64, i64, i64, i64, i64, i64),
    b: &(i64, u32, String, Option<String>, Option<String>, Option<f64>, i64, i64, i64, i64, i64, i64),
) -> bool {
    a.2.trim() == b.2.trim()
        && a.3.as_deref().unwrap_or("").trim() == b.3.as_deref().unwrap_or("").trim()
        && b.6 <= 0
        && b.7 <= 0
        && b.8 <= 0
}

fn query_red_headers(
    conn: &Connection,
    start_date: &str,
    end_date: &str,
) -> Result<Vec<RedHeader>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT h.id, h.inv_unq_key, h.rev_org_key
         FROM invoice_headers h
         WHERE h.is_pos = 0
           AND h.issue_date BETWEEN ?1 AND ?2
           AND h.rev_org_key IS NOT NULL
           AND TRIM(h.rev_org_key) <> ''
           AND EXISTS (
               SELECT 1
               FROM invoice_details d
               WHERE d.header_id = h.id
                 AND NOT EXISTS (
                     SELECT 1
                     FROM reversal_line_matches m
                     WHERE m.red_detail_id = d.id
                       AND m.match_status = ?3
                 )
           )
         ORDER BY h.id",
    )?;

    let rows = stmt.query_map(params![start_date, end_date, MATCH_STATUS_NORMAL], |row| {
        Ok(RedHeader {
            id: row.get(0)?,
            inv_unq_key: row.get(1)?,
            rev_org_key: row.get(2)?,
        })
    })?;

    rows.collect()
}

fn query_red_details(
    conn: &Connection,
    red_header_id: i64,
) -> Result<Vec<RedDetail>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT d.id, d.line_no, d.goods_name, d.spec_model, d.unit_price,
                CAST(d.quantity AS REAL), d.amount, d.tax, d.total
         FROM invoice_details d
         WHERE d.header_id = ?1
           AND NOT EXISTS (
               SELECT 1
               FROM reversal_line_matches m
               WHERE m.red_detail_id = d.id
                 AND m.match_status = ?2
           )
         ORDER BY d.line_no ASC",
    )?;

    let rows = stmt.query_map(params![red_header_id, MATCH_STATUS_NORMAL], |row| {
        Ok(RedDetail {
            id: row.get(0)?,
            line_no: row.get(1)?,
            goods_name: row.get(2)?,
            spec_model: row.get(3)?,
            unit_price: row.get(4)?,
            quantity: row.get(5)?,
            amount: row.get(6)?,
            tax: row.get(7)?,
            total: row.get(8)?,
        })
    })?;

    rows.collect()
}

fn query_blue_header(
    conn: &Connection,
    inv_unq_key: &str,
) -> Result<Option<BlueHeader>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT h.id, h.inv_unq_key, h.state, h.total,
                h.issue_date, s.tax_no, s.name
         FROM invoice_headers h
         JOIN sellers s ON s.id = h.seller_id
         WHERE h.inv_unq_key = ?1
           AND h.is_pos = 1
         LIMIT 1",
    )?;

    let mut rows = stmt.query_map(params![inv_unq_key], |row| {
        Ok(BlueHeader {
            id: row.get(0)?,
            inv_unq_key: row.get(1)?,
            state: row.get(2)?,
            total: row.get(3)?,
            issue_date: row.get(4)?,
            seller_tax_no: row.get(5)?,
            seller_name: row.get(6)?,
        })
    })?;

    rows.next().transpose()
}

fn query_existing_blue_matched_total(
    conn: &Connection,
    blue_header_id: i64,
) -> Result<i64, rusqlite::Error> {
    conn.query_row(
        "SELECT COALESCE(SUM(matched_total), 0)
         FROM reversal_line_matches
         WHERE blue_header_id = ?1
           AND blue_detail_id IS NOT NULL
           AND match_status = ?2",
        params![blue_header_id, MATCH_STATUS_NORMAL],
        |row| row.get(0),
    )
}

fn query_blue_units_with_remaining(
    conn: &Connection,
    blue_header_id: i64,
) -> Result<Vec<BlueUnit>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT d.id, d.line_no, d.goods_name, d.spec_model, d.unit_price,
                CAST(d.quantity AS REAL), d.amount, d.tax, d.total,
                COALESCE(SUM(CASE WHEN m.match_status = ?2 THEN m.matched_total ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN m.match_status = ?2 THEN m.matched_amount ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN m.match_status = ?2 THEN m.matched_tax ELSE 0 END), 0)
         FROM invoice_details d
         LEFT JOIN reversal_line_matches m ON m.blue_detail_id = d.id
         WHERE d.header_id = ?1
         GROUP BY d.id
         ORDER BY d.line_no ASC",
    )?;

    type RawRow = (
        i64,
        u32,
        String,
        Option<String>,
        Option<String>,
        Option<f64>,
        i64,
        i64,
        i64,
        i64,
        i64,
        i64,
    );

    let raw: Vec<RawRow> = stmt
        .query_map(params![blue_header_id, MATCH_STATUS_NORMAL], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
                row.get(7)?,
                row.get(8)?,
                row.get(9)?,
                row.get(10)?,
                row.get(11)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let mut units = Vec::new();
    let mut index = 0;

    while index < raw.len() {
        let row = &raw[index];
        let (
            id,
            line_no,
            goods_name,
            spec_model,
            unit_price,
            quantity,
            amount,
            tax,
            total,
            matched_total,
            matched_amount,
            matched_tax,
        ) = row.clone();

        if let Some(next) = raw.get(index + 1) {
            if is_discount_pair(row, next) {
                let combined_total = total + next.8;
                let combined_amount = amount + next.6;
                let combined_tax = tax + next.7;
                let combined_matched_total = matched_total + next.9;
                let combined_matched_amount = matched_amount + next.10;
                let combined_matched_tax = matched_tax + next.11;

                units.push(BlueUnit {
                    id,
                    line_no,
                    goods_name,
                    spec_model,
                    unit_price: normalize_unit_price(combined_amount, quantity),
                    quantity,
                    amount: combined_amount,
                    tax: combined_tax,
                    total: combined_total,
                    remaining: combined_total - combined_matched_total,
                    remaining_amount: combined_amount - combined_matched_amount,
                    remaining_tax: combined_tax - combined_matched_tax,
                    discount_detail_id: Some(next.0),
                });

                index += 2;
                continue;
            }
        }

        units.push(BlueUnit {
            id,
            line_no,
            goods_name,
            spec_model,
            unit_price,
            quantity,
            amount,
            tax,
            total,
            remaining: total - matched_total,
            remaining_amount: amount - matched_amount,
            remaining_tax: tax - matched_tax,
            discount_detail_id: None,
        });

        index += 1;
    }

    Ok(units)
}

fn insert_match_records(
    conn: &Connection,
    records: &[MatchRecord],
) -> Result<u32, rusqlite::Error> {
    let mut ok_inserted = 0_u32;
    let mut stmt = conn.prepare(
        "INSERT OR IGNORE INTO reversal_line_matches (
            red_header_id, red_detail_id, blue_header_id, blue_detail_id,
            blue_discount_detail_id, matched_quantity, matched_amount,
            matched_tax, matched_total, match_result_type, match_status,
            match_note, match_batch_no
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
    )?;

    for record in records {
        let changed = stmt.execute(params![
            record.red_header_id,
            record.red_detail_id,
            record.blue_header_id,
            record.blue_detail_id,
            record.blue_discount_detail_id,
            record.matched_quantity,
            record.matched_amount,
            record.matched_tax,
            record.matched_total,
            record.match_result_type.to_db_value(),
            record.match_status,
            record.match_note.as_deref(),
            record.match_batch_no,
        ])?;

        if changed > 0 && record.match_result_type == MatchResultType::Ok {
            ok_inserted += 1;
        }
    }

    Ok(ok_inserted)
}

fn update_blue_states(
    conn: &Connection,
    blue_headers: &HashMap<i64, BlueHeader>,
) -> Result<(), rusqlite::Error> {
    for blue_id in blue_headers.keys() {
        let blue_total: i64 = conn.query_row(
            "SELECT total FROM invoice_headers WHERE id = ?1",
            params![blue_id],
            |row| row.get(0),
        )?;

        let matched_total: i64 = conn.query_row(
            "SELECT COALESCE(SUM(matched_total), 0)
             FROM reversal_line_matches
             WHERE blue_header_id = ?1
               AND blue_detail_id IS NOT NULL
               AND match_status = ?2",
            params![blue_id, MATCH_STATUS_NORMAL],
            |row| row.get(0),
        )?;

        if matched_total <= 0 {
            continue;
        }

        let current_state: i64 = conn.query_row(
            "SELECT state FROM invoice_headers WHERE id = ?1",
            params![blue_id],
            |row| row.get(0),
        )?;

        let net_remaining = blue_total - matched_total;
        let target_state = if net_remaining.abs() <= 1 {
            INVOICE_STATE_REVERSED_FULL
        } else {
            INVOICE_STATE_REVERSED_PARTIAL
        };

        let new_state = match (current_state, target_state) {
            (INVOICE_STATE_NORMAL, target) => target,
            (INVOICE_STATE_REVERSED_PARTIAL, INVOICE_STATE_REVERSED_FULL) => {
                INVOICE_STATE_REVERSED_FULL
            }
            (INVOICE_STATE_REVERSED_PARTIAL, INVOICE_STATE_REVERSED_PARTIAL) => {
                INVOICE_STATE_REVERSED_PARTIAL
            }
            _ => current_state,
        };

        if new_state != current_state {
            conn.execute(
                "UPDATE invoice_headers SET state = ?1 WHERE id = ?2",
                params![new_state, blue_id],
            )?;
        }
    }

    Ok(())
}

/// 生成本次匹配运行的内部标识（仅用于把同一次运行产生的记录归到一起，不作为审计依据，
/// 也不需要用户输入）。
fn next_match_run_no(conn: &Connection) -> Result<i32, rusqlite::Error> {
    conn.query_row(
        "SELECT COALESCE(MAX(match_batch_no), 0) + 1 FROM reversal_line_matches",
        [],
        |row| row.get(0),
    )
}

fn delete_stale_external_history(
    conn: &Connection,
    red_detail_ids: &[i64],
) -> Result<(), rusqlite::Error> {
    let mut stmt = conn.prepare(
        "DELETE FROM reversal_line_matches WHERE red_detail_id = ?1 AND match_status = ?2",
    )?;

    for red_detail_id in red_detail_ids {
        stmt.execute(params![red_detail_id, MATCH_STATUS_EXTERNAL_HISTORY])?;
    }

    Ok(())
}
