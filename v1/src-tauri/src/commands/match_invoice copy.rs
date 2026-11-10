// src-tauri/src/commands/match_invoice.rs
//
// 红冲核对 command
// 入参：batch_no（本批次导入批次号，用于定位本批次红字发票）
// 出参：ReversalMatchSummary（成功数、异常数、无蓝票数 + 异常列表）
//
// 匹配算法：
//   1. 取本批次所有红字发票头（is_pos=0, batch_no=?）
//   2. 对每张红票，通过 rev_org_key 找蓝票头
//      → 找不到：整张红票所有明细记录 BlueMissing
//   3. 查蓝票所有明细，计算各行剩余可冲金额（原始 - 已匹配）
//  取消此项 4. 特定业务类型（biz_type IS NOT NULL）：整票头级别处理，不做行级匹配
//   5. 普通明细：按 item_key(goods_name, spec_model, unit_price) 分组
//      → 每组红票N行 vs 蓝票M行
//      → N==1：优先找 remaining >= |red_amount| 的行（取最小满足），找不到则跨行凑
//      → N>=2：按 line_no 顺序一一对应
//   6. 检查蓝票状态异常（原始状态=ReversedFull 但净额不为0）
//   7. 更新蓝票 state（只允许升级：Normal→Partial, Normal/Partial→Full）
//   8. 批量插入 reversal_line_matches

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use specta::Type;
use strum::EnumString;

use crate::DbState;

// ─────────────────────────────────────────────────────────────
// match_result_type 枚举（存库用 u8，前端用 camelCase string）
// ─────────────────────────────────────────────────────────────
#[repr(u8)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type, EnumString)]
#[serde(rename_all = "camelCase")]
pub enum MatchResultType {
    #[strum(serialize = "成功")]
    Ok = 1,
    #[strum(serialize = "无蓝字发票")]
    BlueMissing = 2,
    #[strum(serialize = "红字发票异常")]
    Abnormal = 3,
}

// ─────────────────────────────────────────────────────────────
// 内部工作结构体
// ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct RedHeader {
    id: i64,
    inv_unq_key: String,
    rev_org_key: String,
    total: i64, // 负数
}

#[derive(Debug, Clone)]
struct RedDetail {
    id: i64,
    line_no: u32,
    goods_name: String,
    spec_model: Option<String>,
    unit_price: Option<String>,
    quantity: Option<f64>,
    amount: i64, // 负数
    tax: i64,    // 负数
    total: i64,  // 负数
    biz_type: Option<i64>,
}

#[derive(Debug, Clone)]
struct BlueHeader {
    id: i64,
    inv_unq_key: String,
    state: i64,
    total: i64, // 正数
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
    unit_price: Option<String>, // 无折扣=原始单价；有折扣=折后有效单价
    quantity: Option<f64>,
    amount: i64,
    tax: i64,
    total: i64,
    remaining: i64,
    remaining_amount: i64,
    remaining_tax: i64,
    discount_detail_id: Option<i64>,
    has_discount: bool,
}

#[derive(Debug)]
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
    match_note: Option<String>,
    match_batch_no: i32,
}

// ─────────────────────────────────────────────────────────────
// 对外 DTO
// ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ReversalAbnormalItem {
    pub red_inv_unq_key: String,
    pub red_line_no: u32,
    pub match_result_type: MatchResultType,
    pub match_note: Option<String>,
    // 蓝票信息（BlueMissing 时为 None）
    pub blue_inv_unq_key: Option<String>,
    pub blue_issue_date: Option<String>,
    pub blue_seller_tax_no: Option<String>,
    pub blue_seller_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ReversalMatchSummary {
    pub batch_no: i32,
    pub ok_count: u32,
    pub blue_missing_count: u32,
    pub abnormal_count: u32,
    pub abnormal_items: Vec<ReversalAbnormalItem>,
}

// ─────────────────────────────────────────────────────────────
// Tauri command 入口
// ─────────────────────────────────────────────────────────────

#[tauri::command]
#[specta::specta]
pub async fn match_reversal_invoices(
    batch_no: i32,
    db: tauri::State<'_, DbState>,
) -> Result<ReversalMatchSummary, String> {
    let conn_arc = Arc::clone(&db.conn);
    tokio::task::spawn_blocking(move || {
        let conn = conn_arc.lock().unwrap();
        run_match(&conn, batch_no)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e: rusqlite::Error| e.to_string())
}

// ─────────────────────────────────────────────────────────────
// 核心匹配逻辑
// ─────────────────────────────────────────────────────────────

fn run_match(conn: &Connection, batch_no: i32) -> Result<ReversalMatchSummary, rusqlite::Error> {
    // ── Step 1: 取本批次所有红字发票头 ──────────────────────────
    let red_headers = query_red_headers(conn, batch_no)?;

    let mut all_records: Vec<MatchRecord> = Vec::new();
    let mut abnormal_items: Vec<ReversalAbnormalItem> = Vec::new();
    // let mut ok_count: u32 = 0;
    let mut blue_missing_count: u32 = 0;
    let mut abnormal_count: u32 = 0;

    // 记录本次核对涉及的蓝票 id，用于最后更新 state
    // key: blue_header_id, value: 该蓝票所有明细的净剩余合计（用于判断全额/部分）
    let mut blue_headers_to_update: HashMap<i64, BlueHeader> = HashMap::new();

    for red_hdr in &red_headers {
        // ── Step 2: 查红票明细 ────────────────────────────────────
        let red_details = query_red_details(conn, red_hdr.id)?;

        // ── Step 3: 查蓝票头 ─────────────────────────────────────
        let blue_hdr_opt = query_blue_header(conn, &red_hdr.rev_org_key)?;

        let blue_hdr = match blue_hdr_opt {
            None => {
                // 找不到蓝票：所有红票明细记录 BlueMissing
                for rd in &red_details {
                    all_records.push(MatchRecord {
                        red_header_id: red_hdr.id,
                        red_detail_id: rd.id,
                        blue_header_id: 0, // 无效占位，插入时跳过
                        blue_detail_id: 0,
                        blue_discount_detail_id: None,
                        matched_quantity: None,
                        matched_amount: 0,
                        matched_tax: 0,
                        matched_total: 0,
                        match_result_type: MatchResultType::BlueMissing,
                        match_note: Some(format!("找不到蓝字发票：{}", red_hdr.rev_org_key)),
                        match_batch_no: batch_no,
                    });
                    blue_missing_count += 1;
                    abnormal_items.push(ReversalAbnormalItem {
                        red_inv_unq_key: red_hdr.inv_unq_key.clone(),
                        red_line_no: rd.line_no,
                        match_result_type: MatchResultType::BlueMissing,
                        match_note: Some(format!("找不到蓝字发票：{}", red_hdr.rev_org_key)),
                        blue_inv_unq_key: None,
                        blue_issue_date: None,
                        blue_seller_tax_no: None,
                        blue_seller_name: None,
                    });
                }
                continue;
            }
            Some(h) => h,
        };

        // ── Step 4: 查蓝票明细，计算各行剩余可冲金额 ──────────────
        let mut blue_units = query_blue_units_with_remaining(conn, blue_hdr.id)?;

        // ── Step 5: 检查蓝票状态异常
        //    蓝票原始状态=ReversedFull(3) 但 净额不为0（尾差 > 0.001元 = 100分 * 0.001 = 0分，
        //    实际用 1分 作为尾差容忍，即 |net| > 1）
        //    注意：这里的净额是基于已有匹配记录计算的，不含本批次
        // let existing_matched_total: i64 = blue_units.iter().map(|u| u.total - u.remaining).sum();
        // let net_total = blue_hdr.total + existing_matched_total; // 蓝正+红负，理论上趋近0
        //                                                          // ReversedFull=3
        // if blue_hdr.state == 3 && net_total.abs() > 1 {
        //     // 蓝票标记全额但金额不对，记录异常，但仍继续匹配
        //     abnormal_count += 1;
        //     // 对该红票所有明细都记录异常
        //     for rd in &red_details {
        //         abnormal_items.push(ReversalAbnormalItem {
        //             red_inv_unq_key: red_hdr.inv_unq_key.clone(),
        //             red_line_no: rd.line_no,
        //             match_result_type: MatchResultType::Abnormal,
        //             match_note: Some(format!(
        //                 "蓝票状态为已红冲-全额，但净额为{}分（非0），数据可能有误",
        //                 net_total
        //             )),
        //             blue_inv_unq_key: Some(blue_hdr.inv_unq_key.clone()),
        //             blue_issue_date: Some(blue_hdr.issue_date.clone()),
        //             blue_seller_tax_no: Some(blue_hdr.seller_tax_no.clone()),
        //             blue_seller_name: Some(blue_hdr.seller_name.clone()),
        //         });
        //     }
        //     // 异常情况仍继续匹配，不 continue
        // }

        let records = match_normal(&red_hdr, &red_details, &blue_hdr, &mut blue_units, batch_no);

        // ok_count += records
        //     .iter()
        //     .filter(|r| r.match_result_type == MatchResultType::Ok)
        //     .count() as u32;

        all_records.extend(records);

        // 记录蓝票待更新
        blue_headers_to_update
            .entry(blue_hdr.id)
            .or_insert(blue_hdr);
    }

    // ── Step 7: 批量插入匹配记录 ─────────────────────────────────
    // insert_match_records(conn, &all_records)?;
    let ok_count = insert_match_records(conn, &all_records)?;
    // ── Step 8: 更新蓝票 state ───────────────────────────────────
    //    重新计算每张蓝票的净剩余（含本批次新插入的匹配）
    update_blue_states(conn, &blue_headers_to_update)?;

    Ok(ReversalMatchSummary {
        batch_no,
        ok_count,
        blue_missing_count,
        abnormal_count,
        abnormal_items,
    })
}

// ─────────────────────────────────────────────────────────────
// 普通明细匹配（按 item_key 分组）
// ─────────────────────────────────────────────────────────────
fn match_normal(
    red_hdr: &RedHeader,
    red_details: &[RedDetail],
    blue_hdr: &BlueHeader,
    blue_units: &mut Vec<BlueUnit>,
    batch_no: i32,
) -> Vec<MatchRecord> {
    let mut records = Vec::new();

    // 先按基础 key 分组：goods_name + spec_model
    let mut red_groups: HashMap<String, Vec<&RedDetail>> = HashMap::new();
    for rd in red_details {
        let key = item_key(&rd.goods_name, &rd.spec_model);
        red_groups.entry(key).or_default().push(rd);
    }

    for (key, mut red_group) in red_groups {
        // 蓝票只按基础 key 取候选，不在这里强卡单价
        let mut blue_group: Vec<&mut BlueUnit> = blue_units
            .iter_mut()
            .filter(|u| {
                u.amount > 0 && item_key(&u.goods_name, &u.spec_model) == key && u.remaining > 0
            })
            .collect();

        red_group.sort_by_key(|d| d.line_no);
        blue_group.sort_by_key(|u| u.line_no);

        let n = red_group.len();

        if n == 1 {
            let rd = red_group[0];
            let need = rd.total.abs();

            // 候选：remaining 足够的优先
            let mut candidates: Vec<(usize, Option<f64>, i64, u32)> = blue_group
                .iter()
                .enumerate()
                .filter(|(_, u)| u.remaining >= need)
                .map(|(i, u)| (i, price_distance(rd, u), u.remaining, u.line_no))
                .collect();

            // 排序规则：
            // 1. 有可比较单价的，按单价距离升序
            // 2. 单价都不可比时，按 remaining 最小满足优先
            // 3. 再按 line_no
            candidates.sort_by(|a, b| match (&a.1, &b.1) {
                (Some(da), Some(db)) => da
                    .partial_cmp(db)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| a.2.cmp(&b.2))
                    .then_with(|| a.3.cmp(&b.3)),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => a.2.cmp(&b.2).then_with(|| a.3.cmp(&b.3)),
            });

            if let Some((idx, _, _, _)) = candidates.first().copied() {
                let bu = &mut blue_group[idx];
                let take_total = need;
                let (take_amount, take_tax) = split_amount_tax(rd, take_total);
                let qty = matched_quantity(rd.quantity, bu.quantity);

                bu.remaining -= take_total;
                bu.remaining_amount -= take_amount;
                bu.remaining_tax -= take_tax;

                let note = match price_distance(rd, bu) {
                    Some(dist) if dist > 0.0001 => {
                        Some(format!("按同名同规格匹配，单价接近优先，价差={:.8}", dist))
                    }
                    Some(_) => None,
                    None => Some("按同名同规格匹配；该蓝票无数量/无单价，不参考单价".to_string()),
                };

                records.push(MatchRecord {
                    red_header_id: red_hdr.id,
                    red_detail_id: rd.id,
                    blue_header_id: blue_hdr.id,
                    blue_detail_id: bu.id,
                    blue_discount_detail_id: bu.discount_detail_id,
                    matched_quantity: qty,
                    matched_amount: take_amount,
                    matched_tax: take_tax,
                    matched_total: take_total,
                    match_result_type: MatchResultType::Ok,
                    match_note: note,
                    match_batch_no: batch_no,
                });
            } else {
                // 找不到 remaining 足够的单行，退化为跨行凑
                // 这里也先按“单价更接近”排序，再逐行拿
                blue_group.sort_by(|a, b| {
                    let da = price_distance(rd, a);
                    let db = price_distance(rd, b);

                    match (da, db) {
                        (Some(x), Some(y)) => x
                            .partial_cmp(&y)
                            .unwrap_or(std::cmp::Ordering::Equal)
                            .then_with(|| a.line_no.cmp(&b.line_no)),
                        (Some(_), None) => std::cmp::Ordering::Less,
                        (None, Some(_)) => std::cmp::Ordering::Greater,
                        (None, None) => a.line_no.cmp(&b.line_no),
                    }
                });

                let mut left = need;
                for bu in blue_group.iter_mut() {
                    if left == 0 {
                        break;
                    }
                    if bu.remaining <= 0 {
                        continue;
                    }

                    let take_total = bu.remaining.min(left);
                    let (take_amount, take_tax) = split_amount_tax(rd, take_total);
                    let qty = matched_quantity(rd.quantity, bu.quantity);

                    bu.remaining -= take_total;
                    bu.remaining_amount -= take_amount;
                    bu.remaining_tax -= take_tax;
                    left -= take_total;

                    let note = match price_distance(rd, bu) {
                        Some(dist) if dist > 0.0001 => {
                            Some(format!("跨行凑匹配，单价接近优先，价差={:.8}", dist))
                        }
                        Some(_) => Some("跨行凑匹配".to_string()),
                        None => Some("跨行凑匹配；该蓝票无数量/无单价，不参考单价".to_string()),
                    };

                    records.push(MatchRecord {
                        red_header_id: red_hdr.id,
                        red_detail_id: rd.id,
                        blue_header_id: blue_hdr.id,
                        blue_detail_id: bu.id,
                        matched_quantity: qty,
                        matched_amount: take_amount,
                        matched_tax: take_tax,
                        matched_total: take_total,
                        match_result_type: MatchResultType::Ok,
                        match_note: note,
                        match_batch_no: batch_no,
                        blue_discount_detail_id: bu.discount_detail_id,
                    });
                }
            }
        } else {
            // 多行红票：
            // 这里不能再简单按 line_no 一一 zip
            // 要避免“同名同规格不同单价”错配
            //
            // 做法：对每个 red_detail，依次从 blue_group 里找“最优候选”
            for rd in red_group {
                let need = rd.total.abs();

                let mut candidates: Vec<(usize, Option<f64>, i64, u32)> = blue_group
                    .iter()
                    .enumerate()
                    .filter(|(_, u)| u.remaining > 0)
                    .map(|(i, u)| {
                        (
                            i,
                            price_distance(rd, u),
                            if u.remaining >= need {
                                u.remaining - need
                            } else {
                                i64::MAX / 4
                            },
                            u.line_no,
                        )
                    })
                    .collect();

                // 排序规则：
                // 1. 单价可比且更接近优先
                // 2. remaining 能覆盖 need 的优先（通过差值更小体现）
                // 3. line_no 更早优先
                candidates.sort_by(|a, b| match (&a.1, &b.1) {
                    (Some(da), Some(db)) => da
                        .partial_cmp(db)
                        .unwrap_or(std::cmp::Ordering::Equal)
                        .then_with(|| a.2.cmp(&b.2))
                        .then_with(|| a.3.cmp(&b.3)),
                    (Some(_), None) => std::cmp::Ordering::Less,
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    (None, None) => a.2.cmp(&b.2).then_with(|| a.3.cmp(&b.3)),
                });

                if let Some((idx, _, _, _)) = candidates.first().copied() {
                    let bu = &mut blue_group[idx];
                    let take_total = need.min(bu.remaining);
                    let (take_amount, take_tax) = split_amount_tax(rd, take_total);
                    let qty = matched_quantity(rd.quantity, bu.quantity);

                    bu.remaining -= take_total;
                    bu.remaining_amount -= take_amount;
                    bu.remaining_tax -= take_tax;

                    let note = match price_distance(rd, bu) {
                        Some(dist) if dist > 0.0001 => {
                            Some(format!("多行匹配，单价接近优先，价差={:.8}", dist))
                        }
                        Some(_) => None,
                        None => Some("多行匹配；该蓝票无数量/无单价，不参考单价".to_string()),
                    };

                    records.push(MatchRecord {
                        red_header_id: red_hdr.id,
                        red_detail_id: rd.id,
                        blue_header_id: blue_hdr.id,
                        blue_detail_id: bu.id,
                        matched_quantity: qty,
                        matched_amount: take_amount,
                        matched_tax: take_tax,
                        matched_total: take_total,
                        match_result_type: MatchResultType::Ok,
                        match_note: note,
                        match_batch_no: batch_no,
                        blue_discount_detail_id: bu.discount_detail_id,
                    });
                }
            }
        }
    }

    records
}

// ─────────────────────────────────────────────────────────────
// 辅助：按比例从 total 反推 amount 和 tax
// 避免精度问题：amount = total - tax，tax 按红票税率比例
// ─────────────────────────────────────────────────────────────
fn split_amount_tax(rd: &RedDetail, take_total: i64) -> (i64, i64) {
    let orig_total = rd.total.abs();
    if orig_total == 0 {
        return (0, 0);
    }
    // tax 按比例，amount = total - tax
    let take_tax = (rd.tax.abs() as f64 * take_total as f64 / orig_total as f64).round() as i64;
    let take_amount = take_total - take_tax;
    (take_amount, take_tax)
}

// ─────────────────────────────────────────────────────────────
// 辅助：计算匹配数量
// 两边都有数量才计算，否则 None
// ─────────────────────────────────────────────────────────────
fn matched_quantity(red_qty: Option<f64>, blue_qty: Option<f64>) -> Option<f64> {
    match (red_qty, blue_qty) {
        (Some(rq), Some(_)) => Some(rq.abs()),
        _ => None,
    }
}

// ─────────────────────────────────────────────────────────────
// 辅助：生成 item_key
// ─────────────────────────────────────────────────────────────
fn item_key(goods_name: &str, spec_model: &Option<String>) -> String {
    format!(
        "{}|{}",
        goods_name.trim(),
        spec_model.as_deref().unwrap_or("").trim(),
    )
}

// ─────────────────────────────────────────────────────────────
// 数据库查询函数
// ─────────────────────────────────────────────────────────────

fn query_red_headers(conn: &Connection, batch_no: i32) -> Result<Vec<RedHeader>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT h.id, h.inv_unq_key, h.rev_org_key, h.total
         FROM invoice_headers h
         WHERE h.batch_no = ?1
           AND h.is_pos = 0
           AND h.rev_org_key IS NOT NULL",
    )?;
    let rows = stmt.query_map(params![batch_no], |row| {
        Ok(RedHeader {
            id: row.get(0)?,
            inv_unq_key: row.get(1)?,
            rev_org_key: row.get(2)?,
            total: row.get(3)?,
        })
    })?;
    rows.collect()
}

fn query_red_details(
    conn: &Connection,
    red_header_id: i64,
) -> Result<Vec<RedDetail>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id, line_no, goods_name, spec_model, unit_price,
                CAST(quantity AS REAL), amount, tax, total, biz_type
         FROM invoice_details
         WHERE header_id = ?1
         ORDER BY line_no ASC",
    )?;
    let rows = stmt.query_map(params![red_header_id], |row| {
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
            biz_type: row.get(9)?,
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
           AND h.is_pos = 1",
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

/// 查蓝票所有明细，并计算各行剩余可冲金额
/// 剩余 = 原始 - 已匹配（从 reversal_line_matches 聚合）
/// 同时处理负数行：将负数行的金额合并到前一行的 remaining 中
fn query_blue_units_with_remaining(
    conn: &Connection,
    blue_header_id: i64,
) -> Result<Vec<BlueUnit>, rusqlite::Error> {
    // 查所有明细行（含负数行），按 line_no 排序
    let mut stmt = conn.prepare(
        "SELECT d.id, d.line_no, d.goods_name, d.spec_model, d.unit_price,
                CAST(d.quantity AS REAL), d.amount, d.tax, d.total,
                COALESCE(SUM(m.matched_total), 0)  AS matched_total,
                COALESCE(SUM(m.matched_amount), 0) AS matched_amount,
                COALESCE(SUM(m.matched_tax), 0)    AS matched_tax
         FROM invoice_details d
         LEFT JOIN reversal_line_matches m ON m.blue_detail_id = d.id
         WHERE d.header_id = ?1
         GROUP BY d.id
         ORDER BY d.line_no ASC",
    )?;

    let raw: Vec<(
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
    )> = stmt
        .query_map(params![blue_header_id], |row| {
            Ok((
                row.get(0)?,  // id
                row.get(1)?,  // line_no
                row.get(2)?,  // goods_name
                row.get(3)?,  // spec_model
                row.get(4)?,  // unit_price
                row.get(5)?,  // quantity
                row.get(6)?,  // amount
                row.get(7)?,  // tax
                row.get(8)?,  // total
                row.get(9)?,  // matched_total
                row.get(10)?, // matched_amount
                row.get(11)?, // matched_tax
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    // 处理负数行：将负数行合并到前一行的 remaining
    // 规则：如果某行 amount < 0，则将其 total 合并到前一行
    let mut units: Vec<BlueUnit> = Vec::new();
    let mut i = 0;
    while i < raw.len() {
        let r = &raw[i];
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
        ) = r.clone();

        // 检查下一行是否为负数行
        let next_is_neg = raw
            .get(i + 1)
            .map(|n| n.6 < 0) // n.6 = amount
            .unwrap_or(false);

        if next_is_neg {
            let n = &raw[i + 1];
            // 合并：remaining = (正行total + 负行total) - (正行matched + 负行matched)
            let combined_total = total + n.8;
            let combined_amount = amount + n.6;
            let combined_tax = tax + n.7;
            let combined_matched_total = matched_total + n.9;
            let combined_matched_amount = matched_amount + n.10;
            let combined_matched_tax = matched_tax + n.11;
            let net_unit_price = normalize_unit_price(combined_amount, quantity);
            units.push(BlueUnit {
                id,
                line_no,
                goods_name,
                spec_model,
                unit_price: net_unit_price, // 折后有效单价
                quantity,
                amount: combined_amount,
                tax: combined_tax,
                total: combined_total,
                remaining: combined_total - combined_matched_total,
                remaining_amount: combined_amount - combined_matched_amount,
                remaining_tax: combined_tax - combined_matched_tax,
                discount_detail_id: Some(n.0),
                has_discount: true,
            });

            i += 2; // 跳过负数行
        } else {
            let remaining = total - matched_total;
            let remaining_amount = amount - matched_amount;
            let remaining_tax = tax - matched_tax;
            units.push(BlueUnit {
                id,
                line_no,
                goods_name,
                spec_model,
                unit_price, // 原始单价，保持不变
                quantity,
                amount,
                tax,
                total,
                remaining,
                remaining_amount,
                remaining_tax,
                discount_detail_id: None,
                has_discount: false,
            });

            i += 1;
        }
    }

    Ok(units)
}

// ─────────────────────────────────────────────────────────────
// 批量插入匹配记录（BlueMissing 的 blue_header_id/blue_detail_id 为 0，跳过外键）
// ─────────────────────────────────────────────────────────────
fn insert_match_records(
    conn: &Connection,
    records: &[MatchRecord],
) -> Result<u32, rusqlite::Error> {
    let mut inserted: u32 = 0;

    let mut stmt = conn.prepare(
        "INSERT OR IGNORE INTO reversal_line_matches
         (red_header_id, red_detail_id, blue_header_id, blue_detail_id, blue_discount_detail_id,
          matched_quantity, matched_amount, matched_tax, matched_total,
          match_result_type, match_note, match_batch_no)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
    )?;

    for r in records {
        if r.match_result_type == MatchResultType::BlueMissing {
            continue;
        }
        let changed = stmt.execute(params![
            // ✅ execute() 直接返回影响行数
            r.red_header_id,
            r.red_detail_id,
            r.blue_header_id,
            r.blue_detail_id,
            r.blue_discount_detail_id,
            r.matched_quantity,
            r.matched_amount,
            r.matched_tax,
            r.matched_total,
            r.match_result_type.clone() as u8,
            r.match_note,
            r.match_batch_no,
        ])?;
        inserted += changed as u32; // ✅ OR IGNORE 跳过时返回 0，实际插入返回 1
    }

    Ok(inserted)
}

// ─────────────────────────────────────────────────────────────
// 更新蓝票 state
// 规则（只允许升级，不允许降级）：
//   Normal(1)        → ReversedFull(3) 或 ReversedPartial(4)
//   ReversedPartial(4) → ReversedFull(3)
//   ReversedFull(3)  → 不变（即使计算结果是 Partial）
// ─────────────────────────────────────────────────────────────
fn update_blue_states(
    conn: &Connection,
    blue_headers: &HashMap<i64, BlueHeader>,
) -> Result<(), rusqlite::Error> {
    for (blue_id, _blue_hdr) in blue_headers {
        // ✅ 修复：用减法，matched_total 是正数绝对值，需从蓝票 total 中减去
        let net_remaining: i64 = conn.query_row(
            "SELECT h.total - COALESCE(SUM(m.matched_total), 0)
             FROM invoice_headers h
             LEFT JOIN reversal_line_matches m ON m.blue_header_id = h.id
             WHERE h.id = ?1",
            params![blue_id],
            |row| row.get(0),
        )?;

        let current_state: i64 = conn.query_row(
            "SELECT state FROM invoice_headers WHERE id = ?1",
            params![blue_id],
            |row| row.get(0),
        )?;

        let target_state: i64 = if net_remaining.abs() <= 1 { 3 } else { 4 };

        let new_state = match (current_state, target_state) {
            (1, t) => t,
            (4, 3) => 3,
            (4, 4) => 4,
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

//SQL:
/*
SELECT
    m.matched_amount,
    m.matched_total,
    d1.goods_name,
    d1.amount   AS blue_amount,
    d1.total    AS blue_total,
    d2.amount   AS discount_amount,   -- NULL 表示无折扣行
    d2.total    AS discount_total
FROM reversal_line_matches m
JOIN invoice_details d1 ON d1.id = m.blue_detail_id
LEFT JOIN invoice_details d2 ON d2.id = m.blue_discount_detail_id
WHERE m.red_detail_id = ?


*/
fn normalize_unit_price(net_amount: i64, quantity: Option<f64>) -> Option<String> {
    match quantity {
        Some(qty) if qty.abs() > 1e-9 => {
            let net_price = net_amount as f64 / qty;
            // 保留8位小数，统一精度
            Some(format!("{:.8}", net_price))
        }
        _ => None, // 无数量，单价无法计算
    }
}
// RedDetail.unit_price 是 Option<String>，先 parse 再 normalize
fn normalize_price_str(price_str: &Option<String>) -> Option<String> {
    price_str
        .as_ref()?
        .trim()
        .parse::<f64>()
        .ok()
        .map(|p| format!("{:.8}", p))
}
fn parse_price(price_str: &Option<String>) -> Option<f64> {
    price_str.as_ref()?.trim().parse::<f64>().ok()
}

/// 红票是否“可参考单价”
/// 一般来说：有单价即可；数量是否有值不是强制条件
fn red_price(rd: &RedDetail) -> Option<f64> {
    parse_price(&rd.unit_price)
}

/// 蓝票是否“可参考单价”
/// 你特别强调：
/// - 如果蓝票没有数量，那么也一定没有单价
/// - 这种蓝票被红冲时没必要参考单价
///
/// 所以这里明确要求：蓝票必须同时有 quantity 和 unit_price，才参与单价比较
fn blue_price(u: &BlueUnit) -> Option<f64> {
    match (u.quantity, parse_price(&u.unit_price)) {
        (Some(qty), Some(price)) if qty.abs() > 1e-9 => Some(price),
        _ => None,
    }
}

/// 单价距离：越小越好
/// 如果任一方没有可用单价，则返回 None，表示“不参考单价”
fn price_distance(rd: &RedDetail, bu: &BlueUnit) -> Option<f64> {
    let rp = red_price(rd)?;
    let bp = blue_price(bu)?;
    Some((rp - bp).abs())
}
