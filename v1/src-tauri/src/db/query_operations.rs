// src-tauri/src/db/query_operations.rs
//
// 各查询页面的只读查询。统一约定：
//   - 所有筛选条件 AND 组合，文本类字段用 LIKE 模糊匹配
//   - 日期区间用 >= / <= 比较（issue_date / check_time 等均为 YYYY-MM-DD 文本，可直接比较）
//   - 不做服务端分页：前端用虚拟列表一次性渲染全部结果，翻页会有明显的
//     请求延迟/闪烁，桌面查询场景也很少需要分页。查询条件通常会限定
//     开票日期等区间（最常见是一个月），实际命中行数很少超过几万条。
//   - MAX_ROWS 是防御性上限，避免用户忘记收窄条件时一次性拖回超大结果集；
//     命中上限时 total 仍是真实总数，前端据此提示用户缩小查询范围。
//   - id / header_id 等主键一律转换为 u32（specta 对 i64 导出 BigInt，前端用不了）
//   - 金额字段（分）一律转换为 String（避免超出前端 Number 精度）

use rusqlite::{types::{Value, ValueRef}, Connection, Result as SqlResult, Row};

use crate::models::query_dto::*;

const MAX_ROWS: u32 = 50_000;

fn run_query<T>(
    conn: &Connection,
    select_sql: &str,
    where_sql: &str,
    order_sql: &str,
    params: &[Value],
    map_row: impl Fn(&Row) -> SqlResult<T>,
) -> SqlResult<(Vec<T>, u32)> {
    let count_sql = format!("SELECT COUNT(*) FROM ({select_sql}{where_sql})");
    let total: i64 = conn.query_row(&count_sql, rusqlite::params_from_iter(params), |r| {
        r.get(0)
    })?;

    let data_sql = format!("{select_sql}{where_sql} {order_sql} LIMIT {MAX_ROWS}");
    let mut stmt = conn.prepare(&data_sql)?;
    let rows = stmt
        .query_map(rusqlite::params_from_iter(params), map_row)?
        .collect::<SqlResult<Vec<T>>>()?;
    Ok((rows, total as u32))
}

#[derive(Default)]
struct Where {
    clauses: Vec<String>,
    params: Vec<Value>,
}

impl Where {
    fn like(&mut self, col: &str, v: &Option<String>) {
        if let Some(v) = v {
            let t = v.trim();
            if !t.is_empty() {
                self.clauses.push(format!("{col} LIKE ?"));
                self.params.push(Value::Text(format!("%{t}%")));
            }
        }
    }
    // 多选枚举筛选：空列表等同于"全部"，不加条件
    fn in_u8(&mut self, col: &str, v: &Option<Vec<u8>>) {
        if let Some(v) = v.as_ref().filter(|v| !v.is_empty()) {
            let marks = vec!["?"; v.len()].join(",");
            self.clauses.push(format!("{col} IN ({marks})"));
            self.params.extend(v.iter().map(|x| Value::Integer(*x as i64)));
        }
    }
    fn eq_i32(&mut self, col: &str, v: &Option<i32>) {
        if let Some(v) = v {
            self.clauses.push(format!("{col} = ?"));
            self.params.push(Value::Integer(*v as i64));
        }
    }
    fn ge(&mut self, col: &str, v: &Option<String>) {
        if let Some(v) = v {
            let t = v.trim();
            if !t.is_empty() {
                self.clauses.push(format!("{col} >= ?"));
                self.params.push(Value::Text(t.to_string()));
            }
        }
    }
    fn le(&mut self, col: &str, v: &Option<String>) {
        if let Some(v) = v {
            let t = v.trim();
            if !t.is_empty() {
                self.clauses.push(format!("{col} <= ?"));
                self.params.push(Value::Text(t.to_string()));
            }
        }
    }
    fn raw_eq(&mut self, col: &str, v: Value) {
        self.clauses.push(format!("{col} = ?"));
        self.params.push(v);
    }
    fn sql(&self) -> String {
        if self.clauses.is_empty() {
            String::new()
        } else {
            format!(" WHERE {}", self.clauses.join(" AND "))
        }
    }
}

const HEAD_COLS: &str = "h.code, h.num, h.e_num, s.tax_no, s.name, h.issue_date, h.source, \
     h.inv_type, h.state, h.is_pos, h.risk, h.issuer, h.remark";

/// 读取 HEAD_COLS 对应的 13 列（从 offset 开始）
fn head_at(r: &Row, o: usize) -> SqlResult<InvoiceHead> {
    Ok(InvoiceHead {
        code: r.get(o)?,
        num: r.get(o + 1)?,
        e_num: r.get(o + 2)?,
        seller_tax_no: r.get(o + 3)?,
        seller_name: r.get(o + 4)?,
        issue_date: r.get(o + 5)?,
        source: r.get::<_, i64>(o + 6)? as u8,
        inv_type: r.get::<_, i64>(o + 7)? as u8,
        state: r.get::<_, i64>(o + 8)? as u8,
        is_pos: r.get::<_, i64>(o + 9)? as u8,
        risk: r.get::<_, i64>(o + 10)? as u8,
        issuer: r.get(o + 11)?,
        remark: r.get(o + 12)?,
    })
}

fn opt_u8(row: &Row, idx: usize) -> SqlResult<Option<u8>> {
    Ok(row.get::<_, Option<i64>>(idx)?.map(|v| v as u8))
}

fn u32_at(row: &Row, idx: usize) -> SqlResult<u32> {
    Ok(row.get::<_, i64>(idx)? as u32)
}

fn opt_u32_at(row: &Row, idx: usize) -> SqlResult<Option<u32>> {
    Ok(row.get::<_, Option<i64>>(idx)?.map(|v| v as u32))
}

fn money_at(row: &Row, idx: usize) -> SqlResult<String> {
    Ok(row.get::<_, i64>(idx)?.to_string())
}

// ─────────────────────────────────────────────────────────────
// 发票头
// ─────────────────────────────────────────────────────────────
pub fn query_invoice_headers(
    conn: &Connection,
    f: &InvoiceHeaderFilter,
) -> SqlResult<(Vec<InvoiceHeaderRow>, u32)> {
    let mut w = Where::default();
    w.like("h.inv_unq_key", &f.invoice_number);
    w.like("s.name", &f.seller_name);
    w.like("s.tax_no", &f.seller_tax_no);
    w.ge("h.issue_date", &f.issue_date_from);
    w.le("h.issue_date", &f.issue_date_to);
    w.in_u8("h.inv_type", &f.inv_type);
    w.in_u8("h.state", &f.state);
    w.in_u8("h.is_pos", &f.is_pos);
    w.in_u8("h.risk", &f.risk);
    w.in_u8("h.source", &f.source);
    w.like("h.remark", &f.remark);
    w.eq_i32("h.batch_no", &f.batch_no);
    w.like("h.rev_org_key", &f.rev_org_key);
    w.like("h.red_notice_no", &f.red_notice_no);

    let select = "SELECT h.id, h.code, h.num, h.e_num, h.inv_unq_key, h.seller_id, s.tax_no, s.name, \
         h.issue_date, h.amount, h.tax, h.total, h.source, h.inv_type, h.state, h.is_pos, h.risk, \
         h.issuer, h.remark, h.batch_no, h.rev_org_key, h.red_notice_no \
         FROM invoice_headers h JOIN sellers s ON s.id = h.seller_id";

    run_query(
        conn,
        select,
        &w.sql(),
        "ORDER BY h.issue_date DESC, h.id DESC",
        &w.params,
        |r| {
            Ok(InvoiceHeaderRow {
                id: u32_at(r, 0)?,
                code: r.get(1)?,
                num: r.get(2)?,
                e_num: r.get(3)?,
                inv_unq_key: r.get(4)?,
                seller_id: u32_at(r, 5)?,
                seller_tax_no: r.get(6)?,
                seller_name: r.get(7)?,
                issue_date: r.get(8)?,
                amount: money_at(r, 9)?,
                tax: money_at(r, 10)?,
                total: money_at(r, 11)?,
                source: r.get::<_, i64>(12)? as u8,
                inv_type: r.get::<_, i64>(13)? as u8,
                state: r.get::<_, i64>(14)? as u8,
                is_pos: r.get::<_, i64>(15)? as u8,
                risk: r.get::<_, i64>(16)? as u8,
                issuer: r.get(17)?,
                remark: r.get(18)?,
                batch_no: r.get(19)?,
                rev_org_key: r.get(20)?,
                red_notice_no: r.get(21)?,
            })
        },
    )
}

// ─────────────────────────────────────────────────────────────
// 发票明细
// ─────────────────────────────────────────────────────────────
pub fn query_invoice_details(
    conn: &Connection,
    f: &InvoiceDetailFilter,
) -> SqlResult<(Vec<InvoiceDetailRow>, u32)> {
    let mut w = Where::default();
    w.like("h.inv_unq_key", &f.invoice_number);
    w.like("s.name", &f.seller_name);
    w.like("d.goods_name", &f.goods_name);
    w.like("d.goods_category", &f.goods_category);
    w.like("d.spec_model", &f.spec_model);
    w.like("d.unit", &f.unit);
    w.like("d.tax_code", &f.tax_code);
    w.in_u8("d.biz_type", &f.biz_type);
    w.ge("h.issue_date", &f.issue_date_from);
    w.le("h.issue_date", &f.issue_date_to);

    let select = format!(
        "SELECT d.id, d.header_id, d.line_no, {HEAD_COLS}, \
         d.tax_code, d.biz_type, d.goods_category, d.goods_name, d.spec_model, d.unit, \
         d.quantity, d.unit_price, d.amount, d.tax_rate, d.tax, d.total \
         FROM invoice_details d \
         JOIN invoice_headers h ON h.id = d.header_id \
         JOIN sellers s ON s.id = h.seller_id"
    );

    run_query(
        conn,
        &select,
        &w.sql(),
        "ORDER BY h.issue_date DESC, d.header_id DESC, d.line_no ASC",
        &w.params,
        |r| {
            Ok(InvoiceDetailRow {
                id: u32_at(r, 0)?,
                header_id: u32_at(r, 1)?,
                line_no: u32_at(r, 2)?,
                head: head_at(r, 3)?,
                tax_code: r.get(16)?,
                biz_type: opt_u8(r, 17)?,
                goods_category: r.get(18)?,
                goods_name: r.get(19)?,
                spec_model: r.get(20)?,
                unit: r.get(21)?,
                quantity: r.get(22)?,
                unit_price: r.get(23)?,
                amount: money_at(r, 24)?,
                tax_rate: r.get(25)?,
                tax: money_at(r, 26)?,
                total: money_at(r, 27)?,
            })
        },
    )
}

// ─────────────────────────────────────────────────────────────
// 7 个特定行业明细表
// ─────────────────────────────────────────────────────────────
pub fn query_building_details(
    conn: &Connection,
    f: &BuildingDetailFilter,
) -> SqlResult<(Vec<BuildingDetailRow>, u32)> {
    let mut w = Where::default();
    w.like("h.inv_unq_key", &f.invoice_number);
    w.like("s.name", &f.seller_name);
    w.like("d.project_name", &f.project_name);
    w.like("d.service_location", &f.service_location);
    w.like("d.cross_region_no", &f.cross_region_no);
    w.ge("h.issue_date", &f.issue_date_from);
    w.le("h.issue_date", &f.issue_date_to);

    let select = format!(
        "SELECT d.id, d.header_id, d.line_no, {HEAD_COLS}, \
         d.cross_region_no, d.service_location, d.project_name \
         FROM building_details d \
         JOIN invoice_headers h ON h.id = d.header_id \
         JOIN sellers s ON s.id = h.seller_id"
    );

    run_query(
        conn,
        &select,
        &w.sql(),
        "ORDER BY h.issue_date DESC, d.header_id DESC, d.line_no ASC",
        &w.params,
        |r| {
            Ok(BuildingDetailRow {
                id: u32_at(r, 0)?,
                header_id: u32_at(r, 1)?,
                line_no: u32_at(r, 2)?,
                head: head_at(r, 3)?,
                cross_region_no: r.get(16)?,
                service_location: r.get(17)?,
                project_name: r.get(18)?,
            })
        },
    )
}

pub fn query_real_estate_details(
    conn: &Connection,
    f: &RealEstateDetailFilter,
) -> SqlResult<(Vec<RealEstateDetailRow>, u32)> {
    let mut w = Where::default();
    w.like("h.inv_unq_key", &f.invoice_number);
    w.like("s.name", &f.seller_name);
    w.like("d.plate_no", &f.plate_no);
    w.like("d.cert_no", &f.cert_no);
    w.ge("h.issue_date", &f.issue_date_from);
    w.le("h.issue_date", &f.issue_date_to);

    let select = format!(
        "SELECT d.id, d.header_id, d.line_no, {HEAD_COLS}, \
         d.cert_no, d.plate_no \
         FROM real_estate_details d \
         JOIN invoice_headers h ON h.id = d.header_id \
         JOIN sellers s ON s.id = h.seller_id"
    );

    run_query(
        conn,
        &select,
        &w.sql(),
        "ORDER BY h.issue_date DESC, d.header_id DESC, d.line_no ASC",
        &w.params,
        |r| {
            Ok(RealEstateDetailRow {
                id: u32_at(r, 0)?,
                header_id: u32_at(r, 1)?,
                line_no: u32_at(r, 2)?,
                head: head_at(r, 3)?,
                cert_no: r.get(16)?,
                plate_no: r.get(17)?,
            })
        },
    )
}

pub fn query_passenger_details(
    conn: &Connection,
    f: &PassengerDetailFilter,
) -> SqlResult<(Vec<PassengerDetailRow>, u32)> {
    let mut w = Where::default();
    w.like("h.inv_unq_key", &f.invoice_number);
    w.like("s.name", &f.seller_name);
    w.like("d.passenger_name", &f.passenger_name);
    w.like("d.id_no", &f.id_no);
    w.like("d.depart", &f.depart);
    w.like("d.arrival", &f.arrival);
    w.like("d.transport_type", &f.transport_type);
    w.ge("d.travel_date", &f.travel_date_from);
    w.le("d.travel_date", &f.travel_date_to);

    let select = format!(
        "SELECT d.id, d.header_id, d.line_no, {HEAD_COLS}, \
         d.passenger_name, d.id_no, d.travel_date, d.depart, d.arrival, d.seat_class, d.transport_type \
         FROM passenger_details d \
         JOIN invoice_headers h ON h.id = d.header_id \
         JOIN sellers s ON s.id = h.seller_id"
    );

    run_query(
        conn,
        &select,
        &w.sql(),
        "ORDER BY h.issue_date DESC, d.header_id DESC, d.line_no ASC",
        &w.params,
        |r| {
            Ok(PassengerDetailRow {
                id: u32_at(r, 0)?,
                header_id: u32_at(r, 1)?,
                line_no: u32_at(r, 2)?,
                head: head_at(r, 3)?,
                passenger_name: r.get(16)?,
                id_no: r.get(17)?,
                travel_date: r.get(18)?,
                depart: r.get(19)?,
                arrival: r.get(20)?,
                seat_class: r.get(21)?,
                transport_type: r.get(22)?,
            })
        },
    )
}

pub fn query_cargo_details(
    conn: &Connection,
    f: &CargoDetailFilter,
) -> SqlResult<(Vec<CargoDetailRow>, u32)> {
    let mut w = Where::default();
    w.like("h.inv_unq_key", &f.invoice_number);
    w.like("s.name", &f.seller_name);
    w.like("d.vehicle_no", &f.vehicle_no);
    w.like("d.cargo_name", &f.cargo_name);
    w.like("d.depart", &f.depart);
    w.like("d.arrival", &f.arrival);
    w.ge("h.issue_date", &f.issue_date_from);
    w.le("h.issue_date", &f.issue_date_to);

    let select = format!(
        "SELECT d.id, d.header_id, d.line_no, {HEAD_COLS}, \
         d.transport_type, d.vehicle_no, d.depart, d.arrival, d.cargo_name \
         FROM cargo_details d \
         JOIN invoice_headers h ON h.id = d.header_id \
         JOIN sellers s ON s.id = h.seller_id"
    );

    run_query(
        conn,
        &select,
        &w.sql(),
        "ORDER BY h.issue_date DESC, d.header_id DESC, d.line_no ASC",
        &w.params,
        |r| {
            Ok(CargoDetailRow {
                id: u32_at(r, 0)?,
                header_id: u32_at(r, 1)?,
                line_no: u32_at(r, 2)?,
                head: head_at(r, 3)?,
                transport_type: r.get(16)?,
                vehicle_no: r.get(17)?,
                depart: r.get(18)?,
                arrival: r.get(19)?,
                cargo_name: r.get(20)?,
            })
        },
    )
}

pub fn query_air_details(
    conn: &Connection,
    f: &AirDetailFilter,
) -> SqlResult<(Vec<AirDetailRow>, u32)> {
    let mut w = Where::default();
    w.like("h.inv_unq_key", &f.invoice_number);
    w.like("s.name", &f.seller_name);
    w.like("d.passenger_name", &f.passenger_name);
    w.like("d.id_no", &f.id_no);
    w.like("d.flight_no", &f.flight_no);
    w.like("d.depart", &f.depart);
    w.like("d.arrival", &f.arrival);
    w.ge("d.flight_date", &f.flight_date_from);
    w.le("d.flight_date", &f.flight_date_to);

    let select = format!(
        "SELECT d.id, d.header_id, d.line_no, {HEAD_COLS}, \
         d.passenger_name, d.id_no, d.depart, d.arrival, d.carrier, d.flight_no, d.flight_date, d.flight_time \
         FROM air_details d \
         JOIN invoice_headers h ON h.id = d.header_id \
         JOIN sellers s ON s.id = h.seller_id"
    );

    run_query(
        conn,
        &select,
        &w.sql(),
        "ORDER BY h.issue_date DESC, d.header_id DESC, d.line_no ASC",
        &w.params,
        |r| {
            Ok(AirDetailRow {
                id: u32_at(r, 0)?,
                header_id: u32_at(r, 1)?,
                line_no: u32_at(r, 2)?,
                head: head_at(r, 3)?,
                passenger_name: r.get(16)?,
                id_no: r.get(17)?,
                depart: r.get(18)?,
                arrival: r.get(19)?,
                carrier: r.get(20)?,
                flight_no: r.get(21)?,
                flight_date: r.get(22)?,
                flight_time: r.get(23)?,
            })
        },
    )
}

pub fn query_railway_details(
    conn: &Connection,
    f: &RailwayDetailFilter,
) -> SqlResult<(Vec<RailwayDetailRow>, u32)> {
    let mut w = Where::default();
    w.like("h.inv_unq_key", &f.invoice_number);
    w.like("s.name", &f.seller_name);
    w.like("d.passenger_name", &f.passenger_name);
    w.like("d.id_no", &f.id_no);
    w.like("d.train_no", &f.train_no);
    w.like("d.depart", &f.depart);
    w.like("d.arrival", &f.arrival);
    w.ge("d.travel_date", &f.travel_date_from);
    w.le("d.travel_date", &f.travel_date_to);

    let select = format!(
        "SELECT d.id, d.header_id, d.line_no, {HEAD_COLS}, \
         d.passenger_name, d.id_no, d.depart, d.arrival, d.train_no, d.travel_date, d.travel_time \
         FROM railway_details d \
         JOIN invoice_headers h ON h.id = d.header_id \
         JOIN sellers s ON s.id = h.seller_id"
    );

    run_query(
        conn,
        &select,
        &w.sql(),
        "ORDER BY h.issue_date DESC, d.header_id DESC, d.line_no ASC",
        &w.params,
        |r| {
            Ok(RailwayDetailRow {
                id: u32_at(r, 0)?,
                header_id: u32_at(r, 1)?,
                line_no: u32_at(r, 2)?,
                head: head_at(r, 3)?,
                passenger_name: r.get(16)?,
                id_no: r.get(17)?,
                depart: r.get(18)?,
                arrival: r.get(19)?,
                train_no: r.get(20)?,
                travel_date: r.get(21)?,
                travel_time: r.get(22)?,
            })
        },
    )
}

pub fn query_toll_details(
    conn: &Connection,
    f: &TollDetailFilter,
) -> SqlResult<(Vec<TollDetailRow>, u32)> {
    let mut w = Where::default();
    w.like("h.inv_unq_key", &f.invoice_number);
    w.like("s.name", &f.seller_name);
    w.like("d.plate_no", &f.plate_no);
    w.like("d.vehicle_type", &f.vehicle_type);
    w.ge("d.toll_date_from", &f.toll_date_from);
    w.le("d.toll_date_to", &f.toll_date_to);

    let select = format!(
        "SELECT d.id, d.header_id, d.line_no, {HEAD_COLS}, \
         d.plate_no, d.vehicle_type, d.toll_date_from, d.toll_date_to \
         FROM toll_details d \
         JOIN invoice_headers h ON h.id = d.header_id \
         JOIN sellers s ON s.id = h.seller_id"
    );

    run_query(
        conn,
        &select,
        &w.sql(),
        "ORDER BY h.issue_date DESC, d.header_id DESC, d.line_no ASC",
        &w.params,
        |r| {
            Ok(TollDetailRow {
                id: u32_at(r, 0)?,
                header_id: u32_at(r, 1)?,
                line_no: u32_at(r, 2)?,
                head: head_at(r, 3)?,
                plate_no: r.get(16)?,
                vehicle_type: r.get(17)?,
                toll_date_from: r.get(18)?,
                toll_date_to: r.get(19)?,
            })
        },
    )
}

// ─────────────────────────────────────────────────────────────
// 认证清单 / 差旅费用（共用 certified_list 表）
// ─────────────────────────────────────────────────────────────
fn map_certified_row(r: &Row) -> SqlResult<CertifiedListRow> {
    Ok(CertifiedListRow {
        id: u32_at(r, 0)?,
        header_id: u32_at(r, 1)?,
        invoice_number: r.get(2)?,
        issue_date: r.get(3)?,
        seller_name: r.get(4)?,
        amount: money_at(r, 5)?,
        tax: money_at(r, 6)?,
        total: money_at(r, 7)?,
        domestic_proof_no: r.get(8)?,
        valid_deduct_amount: money_at(r, 9)?,
        invoice_tag: r.get(10)?,
        check_time: r.get(11)?,
        risk_status: r.get::<_, i64>(12)? as u8,
        tax_period: r.get(13)?,
        is_travel_expense: r.get::<_, i64>(14)? as u8,
        voucher_no: r.get(15)?,
        posted_date: r.get(16)?,
    })
}

const CERTIFIED_SELECT: &str = "SELECT c.id, c.header_id, h.inv_unq_key, h.issue_date, s.name, \
     h.amount, h.tax, h.total, c.domestic_proof_no, c.valid_deduct_amount, c.invoice_tag, \
     c.check_time, c.risk_status, c.tax_period, c.is_travel_expense, c.voucher_no, c.posted_date \
     FROM certified_list c \
     JOIN invoice_headers h ON h.id = c.header_id \
     JOIN sellers s ON s.id = h.seller_id";

pub fn query_certified_list(
    conn: &Connection,
    f: &CertifiedListFilter,
) -> SqlResult<(Vec<CertifiedListRow>, u32)> {
    let mut w = Where::default();
    w.raw_eq("c.is_travel_expense", Value::Integer(0));
    w.like("h.inv_unq_key", &f.invoice_number);
    w.like("s.name", &f.seller_name);
    w.like("c.tax_period", &f.tax_period);
    w.in_u8("c.risk_status", &f.risk_status);
    w.like("c.domestic_proof_no", &f.domestic_proof_no);
    w.like("c.invoice_tag", &f.invoice_tag);
    w.ge("c.check_time", &f.check_time_from);
    w.le("c.check_time", &f.check_time_to);

    run_query(
        conn,
        CERTIFIED_SELECT,
        &w.sql(),
        "ORDER BY c.check_time DESC, c.id DESC",
        &w.params,
        map_certified_row,
    )
}

pub fn query_travel_expense(
    conn: &Connection,
    f: &TravelExpenseFilter,
) -> SqlResult<(Vec<CertifiedListRow>, u32)> {
    let mut w = Where::default();
    w.raw_eq("c.is_travel_expense", Value::Integer(1));
    w.like("h.inv_unq_key", &f.invoice_number);
    w.like("s.name", &f.seller_name);
    w.like("c.voucher_no", &f.voucher_no);
    w.ge("c.posted_date", &f.posted_date_from);
    w.le("c.posted_date", &f.posted_date_to);
    w.like("c.tax_period", &f.tax_period);
    w.in_u8("c.risk_status", &f.risk_status);

    run_query(
        conn,
        CERTIFIED_SELECT,
        &w.sql(),
        "ORDER BY c.posted_date DESC, c.id DESC",
        &w.params,
        map_certified_row,
    )
}

// ─────────────────────────────────────────────────────────────
// 银行回单凭证
// ─────────────────────────────────────────────────────────────
pub fn query_bank_vouchers(
    conn: &Connection,
    f: &BankVoucherFilter,
) -> SqlResult<(Vec<BankVoucherRow>, u32)> {
    let mut w = Where::default();
    w.like("h.inv_unq_key", &f.invoice_number);
    w.like("s.name", &f.seller_name);
    w.like("b.voucher_no", &f.voucher_no);
    w.ge("b.posted_date", &f.posted_date_from);
    w.le("b.posted_date", &f.posted_date_to);
    w.ge("b.received_date", &f.received_date_from);
    w.le("b.received_date", &f.received_date_to);
    w.like("b.prepared_by", &f.prepared_by);

    let select = "SELECT b.id, b.header_id, h.inv_unq_key, h.issue_date, s.name, \
         h.amount, h.tax, h.total, b.voucher_no, b.posted_date, b.received_date, b.prepared_by \
         FROM bank_invoice_vouchers b \
         JOIN invoice_headers h ON h.id = b.header_id \
         JOIN sellers s ON s.id = h.seller_id";

    run_query(
        conn,
        select,
        &w.sql(),
        "ORDER BY b.posted_date DESC, b.id DESC",
        &w.params,
        |r| {
            Ok(BankVoucherRow {
                id: u32_at(r, 0)?,
                header_id: u32_at(r, 1)?,
                invoice_number: r.get(2)?,
                issue_date: r.get(3)?,
                seller_name: r.get(4)?,
                amount: money_at(r, 5)?,
                tax: money_at(r, 6)?,
                total: money_at(r, 7)?,
                voucher_no: r.get(8)?,
                posted_date: r.get(9)?,
                received_date: r.get(10)?,
                prepared_by: r.get(11)?,
            })
        },
    )
}

// ─────────────────────────────────────────────────────────────
// AP 应付凭证
// ─────────────────────────────────────────────────────────────
pub fn query_ap_vouchers(
    conn: &Connection,
    f: &ApVoucherFilter,
) -> SqlResult<(Vec<ApVoucherRowDto>, u32)> {
    let mut w = Where::default();
    w.like("a.invoice_no_display", &f.invoice_no_display);
    w.like("v.vendor_name", &f.vendor_name);
    w.like("v.vendor_code", &f.vendor_code);
    w.like("a.voucher_no", &f.voucher_no);
    w.like("a.po_no", &f.po_no);
    w.like("a.barcode", &f.barcode);
    w.like("a.entry_no", &f.entry_no);
    w.in_u8("a.inv_type", &f.inv_type);
    w.in_u8("a.tax_certified", &f.tax_certified);
    w.ge("a.entry_date", &f.entry_date_from);
    w.le("a.entry_date", &f.entry_date_to);
    w.ge("a.invoice_date", &f.invoice_date_from);
    w.le("a.invoice_date", &f.invoice_date_to);
    w.ge("a.received_date", &f.received_date_from);
    w.le("a.received_date", &f.received_date_to);
    w.like("a.preparer", &f.preparer);
    w.like("a.remark", &f.remark);

    let select = "SELECT a.id, a.header_id, h.inv_unq_key, a.vendor_id, v.vendor_code, v.vendor_name, \
         a.barcode, a.invoice_no_display, a.entry_no, a.amount, a.tax_rate, a.tax, a.total, \
         a.inv_type, a.po_no, a.voucher_no, a.invoice_date, a.entry_date, a.remark, \
         a.tax_certified, a.received_date, a.preparer \
         FROM ap_invoice_vouchers a \
         JOIN vendors v ON v.id = a.vendor_id \
         LEFT JOIN invoice_headers h ON h.id = a.header_id";

    run_query(
        conn,
        select,
        &w.sql(),
        "ORDER BY a.entry_date DESC, a.id DESC",
        &w.params,
        |r| {
            Ok(ApVoucherRowDto {
                id: u32_at(r, 0)?,
                header_id: opt_u32_at(r, 1)?,
                invoice_number: r.get(2)?,
                vendor_id: u32_at(r, 3)?,
                vendor_code: r.get(4)?,
                vendor_name: r.get(5)?,
                barcode: r.get(6)?,
                invoice_no_display: r.get(7)?,
                entry_no: r.get(8)?,
                amount: money_at(r, 9)?,
                tax_rate: r.get(10)?,
                tax: money_at(r, 11)?,
                total: money_at(r, 12)?,
                inv_type: r.get::<_, i64>(13)? as u8,
                po_no: r.get(14)?,
                voucher_no: r.get(15)?,
                invoice_date: r.get(16)?,
                entry_date: r.get(17)?,
                remark: r.get(18)?,
                tax_certified: r.get::<_, i64>(19)? as u8,
                received_date: r.get(20)?,
                preparer: r.get(21)?,
            })
        },
    )
}

// ─────────────────────────────────────────────────────────────
// IC 凭证
// ─────────────────────────────────────────────────────────────
pub fn query_ic_vouchers(
    conn: &Connection,
    f: &IcVoucherFilter,
) -> SqlResult<(Vec<IcVoucherRow>, u32)> {
    let mut w = Where::default();
    w.like("h.inv_unq_key", &f.invoice_number);
    w.like("i.document_no", &f.document_no);
    w.like("i.legal_entity", &f.legal_entity);
    w.like("s.name", &f.seller_name);
    w.like("i.original_po", &f.original_po);
    w.like("i.billing", &f.billing);
    w.ge("i.posted_date", &f.posted_date_from);
    w.le("i.posted_date", &f.posted_date_to);
    w.ge("i.received_date", &f.received_date_from);
    w.le("i.received_date", &f.received_date_to);
    w.like("i.preparer", &f.preparer);

    let select = "SELECT i.id, i.header_id, h.inv_unq_key, s.name, i.legal_entity, i.billing, \
         i.document_no, i.posted_date, i.original_po, i.received_date, i.preparer \
         FROM ic_vouchers i \
         JOIN invoice_headers h ON h.id = i.header_id \
         JOIN sellers s ON s.id = h.seller_id";

    run_query(
        conn,
        select,
        &w.sql(),
        "ORDER BY i.posted_date DESC, i.id DESC",
        &w.params,
        |r| {
            Ok(IcVoucherRow {
                id: u32_at(r, 0)?,
                header_id: u32_at(r, 1)?,
                invoice_number: r.get(2)?,
                seller_name: r.get(3)?,
                legal_entity: r.get(4)?,
                billing: r.get(5)?,
                document_no: r.get(6)?,
                posted_date: r.get(7)?,
                original_po: r.get(8)?,
                received_date: r.get(9)?,
                preparer: r.get(10)?,
            })
        },
    )
}

// ─────────────────────────────────────────────────────────────
// 综合查询：一行 = 一个发票明细 item，关联记录按组挂在 item 上
// ─────────────────────────────────────────────────────────────
use std::collections::HashMap;

/// biz_type → 对应特种发票明细表（成品油 / 农产品没有明细表）
const SP_TABLES: [(u8, &str); 7] = [
    (1, "railway_details"),
    (2, "passenger_details"),
    (3, "toll_details"),
    (6, "cargo_details"),
    (7, "real_estate_details"),
    (8, "air_details"),
    (9, "building_details"),
];

fn recs(conn: &Connection, sql: &str, params: &[Value]) -> SqlResult<Vec<Rec>> {
    let mut stmt = conn.prepare(sql)?;
    let names: Vec<String> = stmt.column_names().iter().map(|s| s.to_string()).collect();
    let rows = stmt
        .query_map(rusqlite::params_from_iter(params), |r| {
            (0..names.len())
                .map(|i| {
                    let v = match r.get_ref(i)? {
                        ValueRef::Null => None,
                        ValueRef::Integer(n) => Some(n.to_string()),
                        ValueRef::Real(x) => Some(x.to_string()),
                        ValueRef::Text(t) => Some(String::from_utf8_lossy(t).into_owned()),
                        ValueRef::Blob(b) => Some(b.iter().map(|x| format!("{x:02x}")).collect()),
                    };
                    Ok((names[i].clone(), v))
                })
                .collect::<SqlResult<Rec>>()
        })?
        .collect::<SqlResult<Vec<_>>>()?;
    Ok(rows)
}

/// 在 base(did, hid) / hb(hid) 两个 CTE 上执行 select；select 的第一列必须叫 k，按它分组。
fn keyed(
    conn: &Connection,
    base: &str,
    params: &[Value],
    select: &str,
) -> SqlResult<HashMap<i64, Vec<Rec>>> {
    let sql = format!("WITH base AS ({base}), hb AS (SELECT DISTINCT hid FROM base) {select}");
    let mut out: HashMap<i64, Vec<Rec>> = HashMap::new();
    for mut rec in recs(conn, &sql, params)? {
        let k = rec.remove("k").flatten().and_then(|k| k.parse().ok()).unwrap_or_default();
        out.entry(k).or_default().push(rec);
    }
    Ok(out)
}

fn text<'a>(r: &'a Rec, k: &str) -> Option<&'a str> {
    r.get(k)?.as_deref()
}

fn int(r: &Rec, k: &str) -> i64 {
    text(r, k).and_then(|v| v.parse().ok()).unwrap_or(0)
}

fn flag(b: bool) -> Option<String> {
    Some(if b { "1" } else { "0" }.to_string())
}

fn field(r: Option<Rec>, k: &str) -> Option<String> {
    r?.get(k).cloned().flatten()
}

fn period6(v: &Option<String>) -> Option<String> {
    let t = v.as_deref()?.trim();
    (t.len() == 6 && t.bytes().all(|b| b.is_ascii_digit())).then(|| t.to_string())
}

pub fn query_item_overview(
    conn: &Connection,
    f: &ItemOverviewFilter,
) -> SqlResult<(Vec<ItemOverviewRow>, u32)> {
    // 只给一端时当作单期。期间已校验为 6 位数字，可以直接拼进 SQL
    let (pf, pt) = (period6(&f.period_from), period6(&f.period_to));
    let period = pf
        .clone()
        .or(pt.clone())
        .zip(pt.or(pf))
        .map(|(a, b)| if a <= b { (a, b) } else { (b, a) });

    let mut joins = String::from(
        "FROM invoice_details d \
         JOIN invoice_headers h ON h.id = d.header_id \
         JOIN sellers s ON s.id = h.seller_id \
         LEFT JOIN certified_list c ON c.header_id = h.id \
         LEFT JOIN bank_invoice_vouchers b ON b.header_id = h.id",
    );
    if let Some((a, z)) = &period {
        for (alias, table) in [
            ("wr", "welfare_transfer_results"),
            ("wo", "welfare_transfer_overrides"),
            ("sr", "stamp_tax_results"),
            ("so", "stamp_tax_overrides"),
        ] {
            joins += &format!(
                " LEFT JOIN {table} {alias} ON {alias}.detail_id = d.id \
                 AND {alias}.period_from = '{a}' AND {alias}.period_to = '{z}'"
            );
        }
    }

    let mut w = Where::default();
    w.ge("h.issue_date", &f.issue_date_from);
    w.le("h.issue_date", &f.issue_date_to);
    w.like("h.inv_unq_key", &f.invoice_number);
    w.like("d.goods_name", &f.goods_name);
    w.like("d.tax_code", &f.tax_code);
    w.in_u8("d.biz_type", &f.biz_type);
    let like = |v: &Option<String>| {
        v.as_deref()
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .map(|t| Value::Text(format!("%{t}%")))
    };
    if let Some(p) = like(&f.seller) {
        w.clauses.push("(s.name LIKE ? OR s.tax_no LIKE ?)".into());
        w.params.extend([p.clone(), p]);
    }
    if let Some(p) = like(&f.voucher_no) {
        w.clauses.push(
            "(EXISTS (SELECT 1 FROM ap_invoice_vouchers a WHERE a.header_id = h.id AND a.voucher_no LIKE ?) \
             OR EXISTS (SELECT 1 FROM ic_vouchers i WHERE i.header_id = h.id AND i.document_no LIKE ?) \
             OR b.voucher_no LIKE ? \
             OR (c.is_travel_expense = 1 AND c.voucher_no LIKE ?))"
                .into(),
        );
        w.params.extend(std::iter::repeat(p).take(4));
    }
    if let Some(v) = &f.certified {
        if v.contains(&1) != v.contains(&0) {
            w.clauses
                .push(if v.contains(&1) { "c.id IS NOT NULL" } else { "c.id IS NULL" }.into());
        }
    }
    // 同一条 AP 记录同时满足凭证号与供应商编码
    let (ap_no, ap_vendor) = (like(&f.ap_voucher_no), like(&f.ap_vendor_code));
    if ap_no.is_some() || ap_vendor.is_some() {
        let mut sql = String::from(
            "EXISTS (SELECT 1 FROM ap_invoice_vouchers a JOIN vendors v ON v.id = a.vendor_id \
             WHERE a.header_id = h.id",
        );
        for (col, p) in [("a.voucher_no", ap_no), ("v.vendor_code", ap_vendor)] {
            if let Some(p) = p {
                sql += &format!(" AND {col} LIKE ?");
                w.params.push(p);
            }
        }
        w.clauses.push(sql + ")");
    }
    if let Some(v) = f.reversal.as_ref().filter(|v| !v.is_empty()) {
        let red = "EXISTS (SELECT 1 FROM reversal_line_matches m WHERE m.red_detail_id = d.id)";
        let blue = "EXISTS (SELECT 1 FROM reversal_line_matches m WHERE m.blue_detail_id = d.id)";
        let parts: Vec<String> = v
            .iter()
            .filter_map(|x| match x {
                1 => Some(red.to_string()),
                2 => Some(blue.to_string()),
                3 => Some(format!("NOT ({red} OR {blue})")),
                _ => None,
            })
            .collect();
        if !parts.is_empty() {
            w.clauses.push(format!("({})", parts.join(" OR ")));
        }
    }
    if period.is_some() {
        w.in_u8("COALESCE(wo.included, wr.id IS NOT NULL)", &f.transfer);
        w.in_u8(
            "CASE WHEN COALESCE(so.included, sr.id IS NOT NULL) = 0 THEN 0 \
             WHEN so.stamp_tax_category_id IS NULL \
              AND (SELECT COUNT(*) FROM stamp_tax_rule_hits x WHERE x.result_id = sr.id) <> 1 THEN 2 \
             ELSE 1 END",
            &f.stamp,
        );
    }
    let where_sql = w.sql();
    let order = format!("ORDER BY h.issue_date DESC, d.header_id DESC, d.line_no ASC LIMIT {MAX_ROWS}");

    let total: i64 = conn.query_row(
        &format!("SELECT COUNT(*) {joins}{where_sql}"),
        rusqlite::params_from_iter(&w.params),
        |r| r.get(0),
    )?;

    let item_sql = format!(
        "SELECT d.id AS id, d.header_id AS header_id, d.line_no, d.tax_code, d.biz_type, \
         d.goods_category, d.goods_name, d.spec_model, d.unit, d.quantity, d.unit_price, \
         d.amount, d.tax_rate, d.tax, d.total, \
         h.inv_unq_key, h.code, h.num, h.e_num, h.issue_date, h.inv_type, h.state, h.is_pos, \
         h.source, h.risk, h.issuer, h.remark, h.batch_no, h.rev_org_key, h.red_notice_no, \
         h.amount AS h_amount, h.tax AS h_tax, h.total AS h_total, \
         s.name AS seller_name, s.tax_no AS seller_tax_no, \
         c.id IS NOT NULL AS cert_status, c.tax_period, c.check_time, c.invoice_tag, \
         c.domestic_proof_no, c.valid_deduct_amount, c.risk_status, c.is_travel_expense, \
         c.voucher_no AS travel_voucher_no, c.posted_date AS travel_posted_date, \
         b.voucher_no AS bank_voucher_no, b.posted_date AS bank_posted_date, \
         b.received_date AS bank_received_date, b.prepared_by AS bank_prepared_by \
         {joins}{where_sql} {order}"
    );
    let mut items = recs(conn, &item_sql, &w.params)?;

    let base = format!("SELECT d.id AS did, d.header_id AS hid {joins}{where_sql} {order}");
    let by_header = |sql: &str| keyed(conn, &base, &w.params, sql);

    let ap = by_header(
        "SELECT hb.hid AS k, a.*, v.vendor_code, v.vendor_name FROM hb \
         JOIN ap_invoice_vouchers a ON a.header_id = hb.hid \
         JOIN vendors v ON v.id = a.vendor_id ORDER BY a.id",
    )?;
    let ic = by_header(
        "SELECT hb.hid AS k, i.* FROM hb JOIN ic_vouchers i ON i.header_id = hb.hid ORDER BY i.id",
    )?;
    let mut sp: HashMap<u8, HashMap<i64, Vec<Rec>>> = HashMap::new();
    for (n, table) in SP_TABLES {
        sp.insert(
            n,
            by_header(&format!(
                "SELECT hb.hid AS k, t.* FROM hb JOIN {table} t ON t.header_id = hb.hid ORDER BY t.line_no"
            ))?,
        );
    }
    // dir: 1 = 本行是红字明细，2 = 本行是蓝字被冲；cp_* 是对方发票
    let rev_cols = "m.id, m.matched_quantity, m.matched_amount, m.matched_tax, m.matched_total, \
         m.match_result_type, m.match_status, m.match_note, m.match_batch_no, \
         m.blue_discount_detail_id, cp.inv_unq_key AS cp_inv, cd.line_no AS cp_line";
    let rev = by_header(&format!(
        "SELECT base.did AS k, 1 AS dir, {rev_cols} FROM base \
         JOIN reversal_line_matches m ON m.red_detail_id = base.did \
         LEFT JOIN invoice_headers cp ON cp.id = m.blue_header_id \
         LEFT JOIN invoice_details cd ON cd.id = m.blue_detail_id \
         UNION ALL \
         SELECT base.did, 2, {rev_cols} FROM base \
         JOIN reversal_line_matches m ON m.blue_detail_id = base.did \
         LEFT JOIN invoice_headers cp ON cp.id = m.red_header_id \
         LEFT JOIN invoice_details cd ON cd.id = m.red_detail_id"
    ))?;

    // ───── 税务结论（按期间）─────
    let mut wth = HashMap::new();
    let mut sth = HashMap::new();
    let (mut wres, mut wov, mut sres, mut sov) =
        (HashMap::new(), HashMap::new(), HashMap::new(), HashMap::new());
    let (mut wt_done, mut st_done) = (false, false);
    if let Some((a, z)) = &period {
        let at = |t: &str| format!("{t}.period_from = '{a}' AND {t}.period_to = '{z}'");
        let rule = "ru.id AS rule_id, ru.rule_category, ru.note, ru.needs_manual_review, \
             ru.seller_tax_id_match_type, ru.seller_tax_id_values, ru.tax_code_match_type, \
             ru.tax_code_values, ru.tax_name_match_type, ru.tax_name_values";
        wres = by_header(&format!(
            "SELECT base.did AS k, r.matched_at FROM base \
             JOIN welfare_transfer_results r ON r.detail_id = base.did WHERE {}",
            at("r")
        ))?;
        wov = by_header(&format!(
            "SELECT base.did AS k, o.included, o.transfer_tax, o.remark FROM base \
             JOIN welfare_transfer_overrides o ON o.detail_id = base.did WHERE {}",
            at("o")
        ))?;
        wth = by_header(&format!(
            "SELECT base.did AS k, {rule} FROM base \
             JOIN welfare_transfer_results r ON r.detail_id = base.did \
             JOIN welfare_transfer_rule_hits x ON x.result_id = r.id \
             JOIN tax_match_rules ru ON ru.id = x.rule_id WHERE {} ORDER BY x.id",
            at("r")
        ))?;
        sres = by_header(&format!(
            "SELECT base.did AS k, r.matched_at FROM base \
             JOIN stamp_tax_results r ON r.detail_id = base.did WHERE {}",
            at("r")
        ))?;
        sov = by_header(&format!(
            "SELECT base.did AS k, o.included, o.stamp_tax_category_id, sc.category_name, sc.rate_bp, o.remark \
             FROM base JOIN stamp_tax_overrides o ON o.detail_id = base.did \
             LEFT JOIN stamp_tax_categories sc ON sc.id = o.stamp_tax_category_id WHERE {}",
            at("o")
        ))?;
        sth = by_header(&format!(
            "SELECT base.did AS k, {rule}, sc.category_name, sc.rate_bp FROM base \
             JOIN stamp_tax_results r ON r.detail_id = base.did \
             JOIN stamp_tax_rule_hits x ON x.result_id = r.id \
             JOIN tax_match_rules ru ON ru.id = x.rule_id \
             LEFT JOIN stamp_tax_categories sc ON sc.id = ru.stamp_tax_category_id \
             WHERE {} ORDER BY x.id",
            at("r")
        ))?;
        // 结果表只记录“命中”，该期间没有任何结果/人工行就视为未计算
        let any = |t: &str| {
            conn.query_row(
                &format!("SELECT EXISTS (SELECT 1 FROM {t} WHERE period_from = '{a}' AND period_to = '{z}')"),
                [],
                |r| r.get::<_, bool>(0),
            )
        };
        wt_done = any("welfare_transfer_results")? || any("welfare_transfer_overrides")?;
        st_done = any("stamp_tax_results")? || any("stamp_tax_overrides")?;
    }

    let none: Vec<Rec> = Vec::new();
    let rows = items
        .iter_mut()
        .map(|item| {
            let (did, hid) = (int(item, "id"), int(item, "header_id"));
            let mut groups: HashMap<String, Vec<Rec>> = HashMap::new();
            for (name, m, key) in [("ap", &ap, hid), ("ic", &ic, hid), ("rev", &rev, did)] {
                groups.insert(name.into(), m.get(&key).cloned().unwrap_or_default());
            }
            let biz = text(item, "biz_type").and_then(|v| v.parse::<u8>().ok());
            if let Some((n, m)) = biz.and_then(|n| sp.get(&n).map(|m| (n, m))) {
                groups.insert(format!("sp{n}"), m.get(&hid).cloned().unwrap_or_default());
            }

            let first = |m: &HashMap<i64, Vec<Rec>>| m.get(&did).and_then(|v| v.first()).cloned();
            let (amount, tax) = (int(item, "amount"), int(item, "tax"));

            // 进项税转出：人工结论优先，否则看系统是否命中
            item.insert("wt_computed".into(), flag(wt_done));
            if wt_done {
                let hits = wth.get(&did).unwrap_or(&none);
                let (ov, res) = (first(&wov), first(&wres));
                let fin = ov
                    .as_ref()
                    .and_then(|o| text(o, "included"))
                    .map_or(res.is_some(), |v| v == "1");
                let fin_tax = ov
                    .as_ref()
                    .and_then(|o| text(o, "transfer_tax"))
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(if fin { tax } else { 0 });
                item.insert("wt_hit_count".into(), Some(hits.len().to_string()));
                item.insert("wt_final".into(), flag(fin));
                item.insert("wt_final_tax".into(), Some(fin_tax.to_string()));
                item.insert("wt_manual".into(), flag(ov.is_some()));
                item.insert(
                    "wt_review".into(),
                    flag(hits.iter().any(|h| text(h, "needs_manual_review") == Some("1"))),
                );
                item.insert("wt_remark".into(), field(ov, "remark"));
                item.insert("wt_matched_at".into(), field(res, "matched_at"));
                groups.insert("wth".into(), hits.clone());
            }

            // 印花税：人工指定类别优先；否则仅命中一条规则时沿用，命中多条待用户确认
            item.insert("st_computed".into(), flag(st_done));
            if st_done {
                let hits = sth.get(&did).unwrap_or(&none);
                let (ov, res) = (first(&sov), first(&sres));
                let fin = ov
                    .as_ref()
                    .and_then(|o| text(o, "included"))
                    .map_or(res.is_some(), |v| v == "1");
                let cat = match &ov {
                    Some(o) if text(o, "stamp_tax_category_id").is_some() => Some(o.clone()),
                    _ if fin && hits.len() == 1 => Some(hits[0].clone()),
                    _ => None,
                }
                .filter(|_| fin);
                item.insert("st_hit_count".into(), Some(hits.len().to_string()));
                item.insert("st_final".into(), flag(fin));
                item.insert("st_pending".into(), flag(fin && cat.is_none()));
                item.insert("st_manual".into(), flag(ov.is_some()));
                item.insert("st_remark".into(), field(ov, "remark"));
                if let Some(c) = cat {
                    let bp = int(&c, "rate_bp");
                    // 税额 = |金额| × 基点 / 100000，四舍五入到分，符号随金额
                    let stamp = (amount.abs() * bp + 50_000) / 100_000 * amount.signum();
                    item.insert("st_category".into(), c.get("category_name").cloned().flatten());
                    item.insert("st_rate_bp".into(), Some(bp.to_string()));
                    item.insert("st_amount".into(), Some(stamp.to_string()));
                }
                groups.insert("sth".into(), hits.clone());
            }
            ItemOverviewRow { item: item.clone(), groups }
        })
        .collect();
    Ok((rows, total as u32))
}

#[cfg(test)]
mod item_overview_tests {
    use super::*;

    fn seeded() -> Connection {
        let mut conn = Connection::open_in_memory().unwrap();
        // init_db 建 FTS5 表需要自定义分词器 simple
        unsafe {
            conn.load_extension_enable().unwrap();
            let dll = concat!(env!("CARGO_MANIFEST_DIR"), "/resources/simple.dll");
            conn.load_extension(dll, None::<&str>).unwrap();
        }
        crate::db::schema::init_db(&mut conn).unwrap();
        conn.execute_batch(
            "INSERT INTO sellers(id, tax_no, name) VALUES (1, 'T1', '销方甲');
             INSERT INTO invoice_headers(id, e_num, seller_id, issue_date, amount, tax, total, source, inv_type, state, is_pos, risk, batch_no)
               VALUES (1, 'E1', 1, '2026-09-05', 10000, 900, 10900, 1, 1, 1, 1, 1, 1),
                      (2, 'E2', 1, '2026-09-06', 500, 45, 545, 1, 1, 1, 1, 1, 1);
             INSERT INTO invoice_details(id, header_id, line_no, biz_type, goods_name, amount, tax_rate, tax, total)
               VALUES (1, 1, 1, 6, '运输', 10000, '9%', 900, 10900), (2, 2, 1, NULL, '办公', 500, '9%', 45, 545);
             INSERT INTO cargo_details(header_id, line_no, vehicle_no) VALUES (1, 1, '沪A');
             INSERT INTO vendors(id, vendor_code, vendor_name) VALUES (1, 'V1', '供应商');
             INSERT INTO ap_invoice_vouchers(header_id, vendor_id, invoice_no_display, amount, tax, total, inv_type, voucher_no, tax_certified, received_date, preparer, content_fingerprint)
               VALUES (1, 1, 'E1', 10000, 900, 10900, 0, 'AP1', 1, '2026-09-08', 'X', zeroblob(32));
             INSERT INTO stamp_tax_categories(id, category_name, rate_bp) VALUES (900, '测试类', 30);
             INSERT INTO tax_match_rules(id, scope, rule_category, stamp_tax_category_id, tax_name_match_type, tax_name_values, source, created_at, rule_signature)
               VALUES (900, 'stamp_tax', '运输', 900, 'eq', '运输', 'manual', '2026-09-01', zeroblob(32));
             INSERT INTO stamp_tax_results(id, detail_id, header_id, period_from, period_to, matched_at) VALUES (1, 1, 1, '202609', '202609', 'now');
             INSERT INTO stamp_tax_rule_hits(result_id, rule_id) VALUES (1, 900);",
        )
        .unwrap();
        conn
    }

    #[test]
    fn groups_filters_and_stamp_tax() {
        let conn = seeded();
        let (rows, total) = query_item_overview(&conn, &ItemOverviewFilter::default()).unwrap();
        assert_eq!((rows.len(), total), (2, 2));
        let r1 = rows.iter().find(|r| r.item["id"].as_deref() == Some("1")).unwrap();
        assert_eq!(r1.groups["ap"].len(), 1);
        assert_eq!(r1.groups["sp6"][0]["vehicle_no"].as_deref(), Some("沪A"));
        assert_eq!(r1.item["wt_computed"].as_deref(), Some("0"));

        let f = ItemOverviewFilter {
            period_from: Some("202609".into()),
            ap_voucher_no: Some("AP".into()),
            stamp: Some(vec![1]),
            ..Default::default()
        };
        let (rows, _) = query_item_overview(&conn, &f).unwrap();
        assert_eq!(rows.len(), 1);
        let it = &rows[0].item;
        assert_eq!(it["st_category"].as_deref(), Some("测试类"));
        assert_eq!(it["st_amount"].as_deref(), Some("3")); // 10000 分 × 0.3‰
        assert_eq!(rows[0].groups["sth"].len(), 1);

        let f = ItemOverviewFilter { reversal: Some(vec![3]), certified: Some(vec![0]), ..Default::default() };
        assert_eq!(query_item_overview(&conn, &f).unwrap().0.len(), 2);
    }
}
