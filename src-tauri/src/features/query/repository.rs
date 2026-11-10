// src-tauri/src/db/query_operations.rs
//
// 各查询页面的只读查询。统一约定：
//   - 所有筛选条件 AND 组合；销方名称 / 发票备注 ≥3 个字符走全文索引（invoice_headers_fts，trigram），1~2 个字符回退 LIKE，其余文本字段用 LIKE 模糊匹配
//   - 日期区间用 >= / <= 比较（issue_date / check_time 等均为 YYYY-MM-DD 文本，可直接比较）
//   - 不做服务端分页：前端用虚拟列表一次性渲染全部结果，翻页会有明显的
//     请求延迟/闪烁，桌面查询场景也很少需要分页。查询条件通常会限定
//     开票日期等区间（最常见是一个月），实际命中行数很少超过几万条。
//   - MAX_ROWS 是防御性上限，避免用户忘记收窄条件时一次性拖回超大结果集；
//     命中上限时 total 仍是真实总数，前端据此提示用户缩小查询范围。
//   - id / header_id 等主键一律转换为 u32（specta 对 i64 导出 BigInt，前端用不了）
//   - 金额字段（分）一律转换为 String（避免超出前端 Number 精度）

use rusqlite::{types::{Value, ValueRef}, Connection, Result as SqlResult, Row};

use super::models::*;

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

/// trigram 要求关键词至少 3 个字符（按字符数，不是字节数）。
const FTS_MIN_CHARS: usize = 3;

/// 把一个搜索内容整体当作短语交给 FTS5：包在双引号里，内部双引号转义成 ""，这样 AND / OR / * 等字符不会被当成语法。
fn fts_phrase(keyword: &str) -> String {
    format!("\"{}\"", keyword.replace('"', "\"\""))
}

/// 销方名称 / 备注的检索条件：返回 (SQL 片段, 参数)；关键词为空返回 None。
/// ≥ 3 字符：`{header}.id IN (SELECT rowid FROM invoice_headers_fts WHERE {fts_col} MATCH ?)`
/// < 3 字符：`{like_col} LIKE ?`（trigram 对 1~2 个字符完全搜不到，必须回退）
pub(crate) fn text_search(
    header: &str,
    fts_col: &str,
    like_col: &str,
    keyword: Option<&str>,
) -> Option<(String, Value)> {
    let t = keyword.map(str::trim).filter(|t| !t.is_empty())?;
    if t.chars().count() >= FTS_MIN_CHARS {
        Some((
            format!("{header}.id IN (SELECT rowid FROM invoice_headers_fts WHERE {fts_col} MATCH ?)"),
            Value::Text(fts_phrase(t)),
        ))
    } else {
        Some((format!("{like_col} LIKE ?"), Value::Text(format!("%{t}%"))))
    }
}

#[derive(Default)]
pub struct Where {
    pub clauses: Vec<String>,
    pub params: Vec<Value>,
}

impl Where {
    /// 销方名称 / 发票备注的检索。关键词 ≥ 3 个字符走 invoice_headers_fts（trigram，中间 / 后缀匹配也能走索引），
    /// 1~2 个字符 trigram 搜不到，回退 LIKE。空串不加条件。
    /// `header` 是发票头的表别名；`fts_col` 是 FTS 列（seller_name / remark）；`like_col` 是回退 LIKE 用的列。
    fn fts(&mut self, header: &str, fts_col: &str, like_col: &str, v: &Option<String>) {
        if let Some((clause, param)) = text_search(header, fts_col, like_col, v.as_deref()) {
            self.clauses.push(clause);
            self.params.push(param);
        }
    }
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
    w.fts("h", "seller_name", "s.name", &f.seller_name);
    w.like("s.tax_no", &f.seller_tax_no);
    w.ge("h.issue_date", &f.issue_date_from);
    w.le("h.issue_date", &f.issue_date_to);
    w.in_u8("h.inv_type", &f.inv_type);
    w.in_u8("h.state", &f.state);
    w.in_u8("h.is_pos", &f.is_pos);
    w.in_u8("h.risk", &f.risk);
    w.in_u8("h.source", &f.source);
    w.fts("h", "remark", "h.remark", &f.remark);
    w.batch_set(&f.batch, None);
    w.like("h.rev_org_key", &f.rev_org_key);
    w.like("h.red_notice_no", &f.red_notice_no);

    let select = "SELECT h.id, h.code, h.num, h.e_num, h.inv_unq_key, h.seller_id, s.tax_no, s.name, \
         h.issue_date, h.amount, h.tax, h.total, h.source, h.inv_type, h.state, h.is_pos, h.risk, \
         h.issuer, h.remark, h.rev_org_key, h.red_notice_no \
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
                rev_org_key: r.get(19)?,
                red_notice_no: r.get(20)?,
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
    w.fts("h", "seller_name", "s.name", &f.seller_name);
    w.like("d.goods_name", &f.goods_name);
    w.like("d.goods_category", &f.goods_category);
    w.like("d.spec_model", &f.spec_model);
    w.like("d.unit", &f.unit);
    w.like("d.tax_code", &f.tax_code);
    w.in_u8("d.biz_type", &f.biz_type);
    w.batch_set(&f.batch, Some(("d.goods_name", "d.tax_code")));
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

    let (mut rows, total) = run_query(
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
                is_discount: false,
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
    )?;
    mark_discount_rows(conn, &mut rows)?;
    Ok((rows, total))
}

/// 给明细行补折扣行标志。
/// 判定要看“上一行”，而查询结果经过筛选，相邻行不一定是发票里的相邻行，
/// 所以另取结果所涉及发票的**全部**明细来判定，再回填；判定函数与红冲查询共用。
fn mark_discount_rows(conn: &Connection, rows: &mut [InvoiceDetailRow]) -> SqlResult<()> {
    use crate::features::reversal_query::{discount_flags, DiscountInput};
    use std::collections::HashSet;

    if rows.is_empty() {
        return Ok(());
    }
    conn.execute_batch(
        "CREATE TEMP TABLE IF NOT EXISTS _dq_h (id INTEGER PRIMARY KEY); DELETE FROM _dq_h;",
    )?;
    {
        let mut ins = conn.prepare("INSERT OR IGNORE INTO _dq_h(id) VALUES (?1)")?;
        for r in rows.iter() {
            ins.execute([r.header_id as i64])?;
        }
    }
    let all: Vec<(i64, i64, String, Option<String>, i64, i64, i64)> = {
        let mut stmt = conn.prepare(
            "SELECT d.id, d.header_id, d.goods_name, d.spec_model, d.amount, d.tax, d.total              FROM invoice_details d JOIN _dq_h t ON t.id = d.header_id              ORDER BY d.header_id, d.line_no",
        )?;
        let v = stmt
            .query_map([], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?))
            })?
            .collect::<SqlResult<Vec<_>>>()?;
        v
    };
    conn.execute_batch("DROP TABLE IF EXISTS _dq_h")?;

    let flags = discount_flags(
        &all.iter()
            .map(|(_, h, name, spec, a, t, tot)| DiscountInput {
                header_id: *h,
                goods_name: name,
                spec_model: spec.as_deref(),
                amount: *a,
                tax: *t,
                total: *tot,
            })
            .collect::<Vec<_>>(),
    );
    let discounts: HashSet<i64> =
        all.iter().zip(flags).filter(|(_, f)| *f).map(|(row, _)| row.0).collect();
    for r in rows.iter_mut() {
        r.is_discount = discounts.contains(&(r.id as i64));
    }
    Ok(())
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
    w.fts("h", "seller_name", "s.name", &f.seller_name);
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
    w.fts("h", "seller_name", "s.name", &f.seller_name);
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
    w.fts("h", "seller_name", "s.name", &f.seller_name);
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
    w.fts("h", "seller_name", "s.name", &f.seller_name);
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
    w.fts("h", "seller_name", "s.name", &f.seller_name);
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
    w.fts("h", "seller_name", "s.name", &f.seller_name);
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
    w.fts("h", "seller_name", "s.name", &f.seller_name);
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
    w.fts("h", "seller_name", "s.name", &f.seller_name);
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
    w.fts("h", "seller_name", "s.name", &f.seller_name);
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
    w.fts("h", "seller_name", "s.name", &f.seller_name);
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
    w.fts("h", "seller_name", "s.name", &f.seller_name);
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

/// 同一明细可能在范围内多个纳税所属期批次都有结果（只有一个会被确认）。
/// 为保证一行一明细，每个模块每条明细只取一个批次：已确认的优先，否则取最新（run id 最大）的。
/// `detail` 是明细 id 的列表达式；返回可直接放在 `run_id = ...` 右侧的标量子查询。
fn pick_run(module: &str, detail: &str, a: &str, z: &str) -> String {
    let (results, overrides, confirmed) = if module == "input_tax_transfer" {
        ("welfare_transfer_results", "welfare_transfer_overrides", "welfare_transfer_confirmed_lines")
    } else {
        ("stamp_tax_results", "stamp_tax_overrides", "stamp_tax_confirmed_lines")
    };
    // SQLite 限制：相关子查询的 ORDER BY 不能引用外层列（报 no such column），也不能把外层列放进
    // FROM (...) 派生表。所以不用 ORDER BY ... LIMIT 1，而是给每个候选批次算排序键
    // 「已确认 × 1e9 + 批次 id」，取最大键再对 1e9 取模还原出批次 id。
    format!(
        "(SELECT MAX((EXISTS (SELECT 1 FROM {confirmed} pc WHERE pc.run_id = tr.id AND pc.detail_id = {detail})) \
                     * 1000000000 + tr.id) % 1000000000 \
          FROM tax_runs tr \
          WHERE tr.module = '{module}' AND tr.filing_to >= '{a}' AND tr.filing_from <= '{z}' \
            AND (EXISTS (SELECT 1 FROM {results} pr WHERE pr.run_id = tr.id AND pr.detail_id = {detail}) \
              OR EXISTS (SELECT 1 FROM {overrides} po WHERE po.run_id = tr.id AND po.detail_id = {detail})))"
    )
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
        for (alias, table, module) in [
            ("wr", "welfare_transfer_results", "input_tax_transfer"),
            ("wo", "welfare_transfer_overrides", "input_tax_transfer"),
            ("sr", "stamp_tax_results", "stamp_tax"),
            ("so", "stamp_tax_overrides", "stamp_tax"),
        ] {
            joins += &format!(
                " LEFT JOIN {table} {alias} ON {alias}.detail_id = d.id AND {alias}.run_id = {}",
                pick_run(module, "d.id", a, z)
            );
        }
        // 确认行 → 批次（凭证信息）。confirmed_lines.detail_id 全局 UNIQUE，每个明细最多一行，不会让结果行数翻倍；
        // 再加 run_id = pick_run(...)：pick_run 已确认的优先，所以确认行所在的 run 一定就是被选中的那个，两边不会错位。
        for (cl, bt, tr, table, module) in [
            ("wcl", "wbt", "wtr", "welfare_transfer_confirmed_lines", "input_tax_transfer"),
            ("scl", "sbt", "str_", "stamp_tax_confirmed_lines", "stamp_tax"),
        ] {
            joins += &format!(
                " LEFT JOIN {table} {cl} ON {cl}.detail_id = d.id AND {cl}.run_id = {} \
                  LEFT JOIN tax_run_batches {bt} ON {bt}.id = {cl}.batch_id \
                  LEFT JOIN tax_runs {tr} ON {tr}.id = {cl}.run_id",
                pick_run(module, "d.id", a, z)
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
    w.batch_set(&f.batch, Some(("d.goods_name", "d.tax_code")));
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
             WHEN so.stamp_tax_category_code IS NULL \
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

    // 批次与凭证信息：只有给了纳税所属期才有（与税务列同一前提）。未确认的明细这些列都是 NULL。
    let batch_cols = if period.is_some() {
        let one = |bt: &str, cl: &str, tr: &str, p: &str| {
            format!(
                ", {cl}.id IS NOT NULL AS {p}_confirmed, {tr}.filing_from AS {p}_filing_from, {tr}.filing_to AS {p}_filing_to, \
                 {bt}.batch_no AS {p}_batch_no, {bt}.confirmed_by AS {p}_confirmed_by, {bt}.confirmed_at AS {p}_confirmed_at, \
                 {bt}.locked_by AS {p}_locked_by, {bt}.locked_at AS {p}_locked_at, {bt}.open_reason AS {p}_open_reason, \
                 {bt}.voucher_no AS {p}_voucher_no, {bt}.posted_date AS {p}_posted_date, {bt}.preparer AS {p}_preparer"
            )
        };
        one("wbt", "wcl", "wtr", "wt") + &one("sbt", "scl", "str_", "st")
    } else {
        String::new()
    };

    let item_sql = format!(
        "SELECT d.id AS id, d.header_id AS header_id, d.line_no, d.tax_code, d.biz_type, \
         d.goods_category, d.goods_name, d.spec_model, d.unit, d.quantity, d.unit_price, \
         d.amount, d.tax_rate, d.tax, d.total, \
         h.inv_unq_key, h.code, h.num, h.e_num, h.issue_date, h.inv_type, h.state, h.is_pos, \
         h.source, h.risk, h.issuer, h.remark, h.rev_org_key, h.red_notice_no, \
         h.amount AS h_amount, h.tax AS h_tax, h.total AS h_total, \
         s.name AS seller_name, s.tax_no AS seller_tax_no, \
         c.id IS NOT NULL AS cert_status, c.tax_period, c.check_time, c.invoice_tag, \
         c.domestic_proof_no, c.valid_deduct_amount, c.risk_status, c.is_travel_expense, \
         c.voucher_no AS travel_voucher_no, c.posted_date AS travel_posted_date, \
         b.voucher_no AS bank_voucher_no, b.posted_date AS bank_posted_date, \
         b.received_date AS bank_received_date, b.prepared_by AS bank_prepared_by{batch_cols} \
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
        let at = |t: &str, module: &str| format!("{t}.run_id = {}", pick_run(module, "base.did", a, z));
        const W: &str = "input_tax_transfer";
        const S: &str = "stamp_tax";
        // enabled / source / created_at：规则只会停用不会删除，命中过的规则现在还有没有效，审计时需要看到
        let rule = "ru.id AS rule_id, ru.rule_category, ru.note, ru.needs_manual_review, \
             ru.enabled AS rule_enabled, ru.source AS rule_source, ru.created_at AS rule_created_at, \
             ru.seller_tax_id_match_type, ru.seller_tax_id_values, ru.tax_code_match_type, \
             ru.tax_code_values, ru.tax_name_match_type, ru.tax_name_values";
        wres = by_header(&format!(
            "SELECT base.did AS k, r.matched_at FROM base \
             JOIN welfare_transfer_results r ON r.detail_id = base.did WHERE {}",
            at("r", W)
        ))?;
        wov = by_header(&format!(
            "SELECT base.did AS k, o.included, o.transfer_tax, o.remark FROM base \
             JOIN welfare_transfer_overrides o ON o.detail_id = base.did WHERE {}",
            at("o", W)
        ))?;
        wth = by_header(&format!(
            "SELECT base.did AS k, {rule} FROM base \
             JOIN welfare_transfer_results r ON r.detail_id = base.did \
             JOIN welfare_transfer_rule_hits x ON x.result_id = r.id \
             JOIN tax_match_rules ru ON ru.id = x.rule_id WHERE {} ORDER BY x.id",
            at("r", W)
        ))?;
        sres = by_header(&format!(
            "SELECT base.did AS k, r.matched_at FROM base \
             JOIN stamp_tax_results r ON r.detail_id = base.did WHERE {}",
            at("r", S)
        ))?;
        sov = by_header(&format!(
            "SELECT base.did AS k, o.included, o.stamp_tax_category_code, sc.name, sc.rate_ppm, o.remark \
             FROM base JOIN stamp_tax_overrides o ON o.detail_id = base.did \
             LEFT JOIN stamp_tax_categories sc ON sc.code = o.stamp_tax_category_code WHERE {}",
            at("o", S)
        ))?;
        sth = by_header(&format!(
            "SELECT base.did AS k, {rule}, sc.name, sc.rate_ppm FROM base \
             JOIN stamp_tax_results r ON r.detail_id = base.did \
             JOIN stamp_tax_rule_hits x ON x.result_id = r.id \
             JOIN tax_match_rules ru ON ru.id = x.rule_id \
             LEFT JOIN stamp_tax_categories sc ON sc.code = ru.stamp_tax_category_code \
             WHERE {} ORDER BY x.id",
            at("r", S)
        ))?;
        // 结果表只记录“命中”，该期间范围内没有任何结果/人工行就视为未计算
        let any = |t: &str| {
            conn.query_row(
                &format!(
                    "SELECT EXISTS (SELECT 1 FROM {t} x JOIN tax_runs tr ON tr.id = x.run_id \
                     WHERE tr.filing_to >= '{a}' AND tr.filing_from <= '{z}')"
                ),
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
                    Some(o) if text(o, "stamp_tax_category_code").is_some() => Some(o.clone()),
                    _ if fin && hits.len() == 1 => Some(hits[0].clone()),
                    _ => None,
                }
                .filter(|c| fin && text(c, "rate_ppm").is_some());
                item.insert("st_hit_count".into(), Some(hits.len().to_string()));
                item.insert("st_final".into(), flag(fin));
                item.insert("st_pending".into(), flag(fin && cat.is_none()));
                item.insert("st_manual".into(), flag(ov.is_some()));
                item.insert("st_remark".into(), field(ov, "remark"));
                if let Some(c) = cat {
                    let bp = int(&c, "rate_ppm");
                    // 基数：专用发票用金额，其余用价税合计；与印花税页同一函数
                    let base = crate::features::stamp_tax::stamp_base(
                        int(item, "inv_type"),
                        amount,
                        int(item, "total"),
                    );
                    let stamp = crate::features::stamp_tax::stamp_amount(base, bp);
                    item.insert("st_category".into(), c.get("name").cloned().flatten());
                    item.insert("st_rate_ppm".into(), Some(bp.to_string()));
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
             INSERT INTO invoice_headers(id, e_num, seller_id, issue_date, amount, tax, total, source, inv_type, state, is_pos, risk)
               VALUES (1, 'E1', 1, '2026-09-05', 10000, 900, 10900, 1, 1, 1, 1, 1),
                      (2, 'E2', 1, '2026-09-06', 500, 45, 545, 1, 1, 1, 1, 1);
             INSERT INTO invoice_details(id, header_id, line_no, biz_type, goods_name, amount, tax_rate, tax, total)
               VALUES (1, 1, 1, 6, '运输', 10000, '9%', 900, 10900), (2, 2, 1, NULL, '办公', 500, '9%', 45, 545);
             INSERT INTO cargo_details(header_id, line_no, vehicle_no) VALUES (1, 1, '沪A');
             INSERT INTO vendors(id, vendor_code, vendor_name) VALUES (1, 'V1', '供应商');
             INSERT INTO ap_invoice_vouchers(header_id, vendor_id, invoice_no_display, amount, tax, total, inv_type, voucher_no, tax_certified, received_date, preparer, content_fingerprint)
               VALUES (1, 1, 'E1', 10000, 900, 10900, 0, 'AP1', 1, '2026-09-08', 'X', zeroblob(32));
             INSERT INTO stamp_tax_categories(code, name, parent_code, rate_ppm) VALUES ('900', '测试类', NULL, 300);
             INSERT INTO tax_match_rules(id, scope, rule_category, stamp_tax_category_code, tax_name_match_type, tax_name_values, source, created_at, rule_signature)
               VALUES (900, 'stamp_tax', '运输', '900', 'eq', '运输', 'manual', '2026-09-01', zeroblob(32));
             INSERT INTO tax_runs(id, module, filing_from, filing_to, src_from, src_to, created_by, created_at)
               VALUES (1, 'stamp_tax', '202609', '202609', '202609', '202609', 'u', 'now');
             INSERT INTO tax_run_batches(id, run_id, batch_no, created_by, created_at) VALUES (1, 1, 1, 'u', 'now');
             INSERT INTO stamp_tax_results(id, run_id, detail_id, header_id, matched_at) VALUES (1, 1, 1, 1, 'now');
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

    /// 批次与凭证信息：已确认的明细带出批次/凭证；未确认的为空；结果行数不因 JOIN 而翻倍；规则带出停用状态/来源/创建时间。
    #[test]
    fn batch_voucher_and_rule_meta() {
        let conn = seeded();
        // 把明细 1 的印花税确认并锁定（批次 1），明细 2 保持未确认
        conn.execute_batch(
            "UPDATE tax_run_batches SET confirmed_by='张三', confirmed_at='2026-10-02',
                    locked_by='李四', locked_at='2026-10-03',
                    voucher_no='SV001', posted_date='2026-10-03', preparer='王五' WHERE id = 1;
             INSERT INTO stamp_tax_confirmed_lines(run_id, batch_id, detail_id, header_id, category_code,
                    base_amount, rate_ppm, stamp_tax, decision, confirmed_at)
               VALUES (1, 1, 1, 1, '900', 10000, 300, 3, 'auto', '2026-10-02');",
        )
        .unwrap();
        // 注意：上面先 UPDATE 再 INSERT 确认行，而锁定触发器只拦截对「已锁定批次」的 UPDATE/DELETE，所以顺序不能反。
        let f = ItemOverviewFilter { period_from: Some("202609".into()), ..Default::default() };
        let (rows, total) = query_item_overview(&conn, &f).unwrap();
        // JOIN 确认行/批次/所属期是 N:1，行数必须仍是 2
        assert_eq!((rows.len(), total), (2, 2));

        let r1 = rows.iter().find(|r| r.item["id"].as_deref() == Some("1")).unwrap();
        assert_eq!(r1.item["st_confirmed"].as_deref(), Some("1"));
        assert_eq!(r1.item["st_batch_no"].as_deref(), Some("1"));
        assert_eq!(r1.item["st_confirmed_by"].as_deref(), Some("张三"));
        assert_eq!(r1.item["st_locked_by"].as_deref(), Some("李四"));
        assert_eq!(r1.item["st_voucher_no"].as_deref(), Some("SV001"));
        assert_eq!(r1.item["st_posted_date"].as_deref(), Some("2026-10-03"));
        assert_eq!(r1.item["st_preparer"].as_deref(), Some("王五"));
        assert_eq!(r1.item["st_filing_from"].as_deref(), Some("202609"));
        // 进项转出模块没有确认行：这些列为 0 / NULL，不能被印花税的批次串过来
        assert_eq!(r1.item["wt_confirmed"].as_deref(), Some("0"));
        assert_eq!(r1.item["wt_batch_no"], None);
        assert_eq!(r1.item["wt_voucher_no"], None);

        let r2 = rows.iter().find(|r| r.item["id"].as_deref() == Some("2")).unwrap();
        assert_eq!(r2.item["st_confirmed"].as_deref(), Some("0"));
        assert_eq!(r2.item["st_batch_no"], None);
        assert_eq!(r2.item["st_voucher_no"], None);

        // 规则元数据
        let hit = &r1.groups["sth"][0];
        assert_eq!(hit["rule_enabled"].as_deref(), Some("1"));
        assert_eq!(hit["rule_source"].as_deref(), Some("manual"));
        assert_eq!(hit["rule_created_at"].as_deref(), Some("2026-09-01"));

        // 没给纳税所属期：不带批次列（与税务列同一前提）
        let (rows, _) = query_item_overview(&conn, &ItemOverviewFilter::default()).unwrap();
        assert!(!rows[0].item.contains_key("st_batch_no"));
    }
}

#[cfg(test)]
mod discount_flag_tests {
    use super::*;
    use crate::features::reversal_query::repository::query_reversal;
    use crate::features::reversal_query::ReversalQueryFilter;

    fn conn() -> Connection {
        let mut c = Connection::open_in_memory().unwrap();
        // init_db 建 FTS5 表需要自定义分词器 simple
        unsafe {
            c.load_extension_enable().unwrap();
            let dll = concat!(env!("CARGO_MANIFEST_DIR"), "/resources/simple.dll");
            c.load_extension(dll, None::<&str>).ok();
        }
        crate::db::schema::init_db(&mut c).unwrap();
        // 发票 1：a 1000 / a 折扣 -100 / a 再一条负数 -50（已是折扣行后不再吸附）/ b 500
        // 发票 2：x 300 / x 折扣 -30
        c.execute_batch(
            "INSERT INTO sellers(id, tax_no, name) VALUES (1,'T1','甲');
             INSERT INTO invoice_headers(id,e_num,seller_id,issue_date,amount,tax,total,source,inv_type,state,is_pos,risk)
               VALUES (1,'E1',1,'2026-09-01',1350,122,1472,1,1,1,1,1),
                      (2,'E2',1,'2026-09-02',270,25,295,1,1,1,1,1);
             INSERT INTO invoice_details(id,header_id,line_no,goods_name,amount,tax_rate,tax,total)
               VALUES (1,1,1,'a',1000,'9%',90,1090),
                      (2,1,2,'a',-100,'9%',-9,-109),
                      (3,1,3,'a',-50,'9%',-4,-54),
                      (4,1,4,'b',500,'9%',45,545),
                      (5,2,1,'x',300,'9%',27,327),
                      (6,2,2,'x',-30,'9%',-2,-32);",
        )
        .unwrap();
        c
    }

    fn flags(c: &Connection, f: &InvoiceDetailFilter) -> Vec<(u32, bool)> {
        let (rows, _) = query_invoice_details(c, f).unwrap();
        let mut v: Vec<_> = rows.iter().map(|r| (r.id, r.is_discount)).collect();
        v.sort();
        v
    }

    #[test]
    fn marks_discount_rows_in_full_result() {
        let c = conn();
        assert_eq!(
            flags(&c, &InvoiceDetailFilter::default()),
            vec![(1, false), (2, true), (3, false), (4, false), (5, false), (6, true)]
        );
    }

    #[test]
    fn filter_does_not_change_the_flag() {
        let c = conn();
        // 只查货物名称 x：折扣行的上一行（id=5）会被筛掉，但 id=6 仍要判成折扣行
        let f = InvoiceDetailFilter { goods_name: Some("x".into()), ..Default::default() };
        assert_eq!(flags(&c, &f), vec![(5, false), (6, true)]);
        // 只查发票 1 的 a：id=2 的上一行 id=1 也在结果里，id=3 紧跟折扣行不再算折扣
        let f = InvoiceDetailFilter { invoice_number: Some("E1".into()), goods_name: Some("a".into()), ..Default::default() };
        assert_eq!(flags(&c, &f), vec![(1, false), (2, true), (3, false)]);
    }

    #[test]
    fn agrees_with_reversal_query_count() {
        let c = conn();
        // 红冲查询只看蓝字发票，折扣行笔数要和发票明细查询里标出的一样多
        let detail = flags(&c, &InvoiceDetailFilter::default()).iter().filter(|(_, d)| *d).count() as u32;
        let rq = query_reversal(&c, &ReversalQueryFilter::default()).unwrap();
        assert_eq!(rq.discount.count, detail);
        assert_eq!(rq.discount.count, 2);
    }
}

/// 销方名称 / 备注：≥3 个字符走 invoice_headers_fts（trigram），1~2 个字符回退 LIKE。
#[cfg(test)]
mod fts_search_tests {
    use super::*;

    fn conn() -> Connection {
        let mut c = Connection::open_in_memory().unwrap();
        // trigram 是 SQLite 自带分词器，不需要加载 simple.dll
        crate::db::schema::init_db(&mut c).unwrap();
        c.execute_batch(
            "INSERT INTO sellers(id, tax_no, name) VALUES
               (1, 'T1', '北京滴滴出行科技有限公司'),
               (2, 'T2', '上海大众交通集团股份有限公司'),
               (3, 'T3', '滴水湖餐饮服务有限公司'),
               (4, 'T4', 'ABC Trading Co.,Ltd');
             INSERT INTO invoice_headers(id, e_num, seller_id, issue_date, amount, tax, total, source, inv_type, state, is_pos, risk, remark)
               VALUES (1,'E1',1,'2026-09-01',100,9,109,1,1,1,1,1,'2026年出差打车费用'),
                      (2,'E2',2,'2026-09-01',100,9,109,1,1,1,1,1,'出租车'),
                      (3,'E3',3,'2026-09-01',100,9,109,1,1,1,1,1,''),
                      (4,'E4',4,'2026-09-01',100,9,109,1,1,1,1,1,'say \"hi\" AND bye');",
        )
        .unwrap();
        c
    }

    fn ids(c: &Connection, seller: Option<&str>, remark: Option<&str>) -> Vec<u32> {
        let f = InvoiceHeaderFilter {
            seller_name: seller.map(Into::into),
            remark: remark.map(Into::into),
            ..Default::default()
        };
        let mut v: Vec<u32> = query_invoice_headers(c, &f).unwrap().0.iter().map(|r| r.id).collect();
        v.sort();
        v
    }

    #[test]
    fn three_or_more_chars_match_prefix_middle_and_suffix_substrings() {
        let c = conn();
        assert_eq!(ids(&c, Some("滴滴出行"), None), vec![1]); // 中间
        assert_eq!(ids(&c, Some("出行科技"), None), vec![1]); // 跨词片段：jieba 做不到，trigram 可以
        assert_eq!(ids(&c, Some("有限公司"), None), vec![1, 2, 3]); // 后缀
        assert_eq!(ids(&c, Some("  滴滴出行  "), None), vec![1]); // 前后空白忽略
        assert!(ids(&c, Some("不存在的公司"), None).is_empty());
    }

    #[test]
    fn one_or_two_chars_fall_back_to_like_instead_of_returning_nothing() {
        let c = conn();
        // trigram 对 <3 字符完全搜不到，必须回退 LIKE，且结果与 LIKE 一致
        assert_eq!(ids(&c, Some("滴滴"), None), vec![1]);
        assert_eq!(ids(&c, Some("滴"), None), vec![1, 3]);
        assert_eq!(ids(&c, None, Some("打车")), vec![1]);
    }

    #[test]
    fn empty_keyword_adds_no_condition() {
        let c = conn();
        assert_eq!(ids(&c, Some(""), None), vec![1, 2, 3, 4]);
        assert_eq!(ids(&c, Some("   "), Some("")), vec![1, 2, 3, 4]);
    }

    #[test]
    fn case_insensitive_and_fts_syntax_chars_are_literal() {
        let c = conn();
        assert_eq!(ids(&c, Some("abc trad"), None), vec![4]); // 大小写不敏感，空格是普通字符
        // 备注里的双引号 / AND 被当成普通文本，不会变成 FTS 语法而报错
        assert_eq!(ids(&c, None, Some("\"hi\" AND")), vec![4]);
        assert_eq!(ids(&c, None, Some("AND bye")), vec![4]);
    }

    #[test]
    fn remark_search_and_seller_rename_stay_in_sync() {
        let c = conn();
        assert_eq!(ids(&c, None, Some("出差打车")), vec![1]);
        assert_eq!(ids(&c, Some("滴滴出行"), Some("出租车")), Vec::<u32>::new()); // 两个条件 AND
        // 销方改名（导入时 ON CONFLICT DO UPDATE SET name）：contentless-delete 表不能只更新部分列，
        // 触发器必须「先删后插」，否则这条 UPDATE 会直接报错
        c.execute("UPDATE sellers SET name = '广州顺风车有限公司' WHERE id = 2", []).unwrap();
        assert_eq!(ids(&c, Some("顺风车"), None), vec![2]);
        assert!(ids(&c, Some("大众交通"), None).is_empty());
        // 发票备注修改同理
        c.execute("UPDATE invoice_headers SET remark = '改成了加油费用' WHERE id = 1", []).unwrap();
        assert_eq!(ids(&c, None, Some("加油费用")), vec![1]);
        assert!(ids(&c, None, Some("出差打车")).is_empty());
        // 删除发票头后索引里也没有了
        c.execute("DELETE FROM invoice_headers WHERE id = 1", []).unwrap();
        assert!(ids(&c, None, Some("加油费用")).is_empty());
    }

    /// 旧库的 FTS 不是 trigram：init_db 必须丢弃旧索引、按新语句重建，并把已有发票头回填进去，
    /// 否则升级后所有旧数据都搜不到。
    #[test]
    fn init_db_rebuilds_a_non_trigram_index_and_backfills_existing_rows() {
        let mut c = Connection::open_in_memory().unwrap();
        crate::db::schema::init_db(&mut c).unwrap();
        c.execute_batch(
            "INSERT INTO sellers(id, tax_no, name) VALUES (1, 'T1', '北京滴滴出行科技有限公司');
             INSERT INTO invoice_headers(id, e_num, seller_id, issue_date, amount, tax, total, source, inv_type, state, is_pos, risk, remark)
               VALUES (1,'E1',1,'2026-09-01',100,9,109,1,1,1,1,1,'出差打车费用');
             -- 退回成旧结构：unicode61 分词的 FTS 表（同样不含 trigram），且里面是空的
             DROP TRIGGER invoice_headers_ai; DROP TRIGGER invoice_headers_au;
             DROP TRIGGER invoice_headers_ad; DROP TRIGGER sellers_au;
             DROP TABLE invoice_headers_fts;
             CREATE VIRTUAL TABLE invoice_headers_fts USING fts5(seller_name, remark, content='', contentless_delete=1, tokenize='unicode61');",
        )
        .unwrap();
        assert_eq!(ids(&c, Some("滴滴出行"), None), Vec::<u32>::new()); // 旧索引是空的，搜不到

        crate::db::schema::init_db(&mut c).unwrap(); // 升级

        assert_eq!(ids(&c, Some("滴滴出行"), None), vec![1]); // 回填后能搜到
        assert_eq!(ids(&c, None, Some("出差打车")), vec![1]);
        let sql: String = c
            .query_row("SELECT sql FROM sqlite_master WHERE name = 'invoice_headers_fts'", [], |r| r.get(0))
            .unwrap();
        assert!(sql.contains("trigram"));
        // 触发器也被重建：新插入的发票头能搜到
        c.execute_batch(
            "INSERT INTO invoice_headers(id, e_num, seller_id, issue_date, amount, tax, total, source, inv_type, state, is_pos, risk, remark)
               VALUES (2,'E2',1,'2026-09-02',100,9,109,1,1,1,1,1,'加油费用');",
        )
        .unwrap();
        assert_eq!(ids(&c, None, Some("加油费用")), vec![2]);
        // 再升级一次是空操作：不会重复回填出两份
        crate::db::schema::init_db(&mut c).unwrap();
        assert_eq!(ids(&c, Some("滴滴出行"), None), vec![1, 2]);
    }
}

/// 手动跑的性能验证（默认忽略）：在真实表结构上灌 50 万发票头，计时应用实际生成的查询。
/// 运行：cargo test --release --lib fts_perf -- --ignored --nocapture
#[cfg(test)]
mod fts_perf {
    use super::*;
    use std::time::Instant;

    #[test]
    #[ignore]
    fn fts_perf_500k_headers() {
        let mut c = Connection::open_in_memory().unwrap();
        crate::db::schema::init_db(&mut c).unwrap();
        let t0 = Instant::now();
        c.execute_batch(
            "WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i+1 FROM n WHERE i<20000)
             INSERT INTO sellers(id, tax_no, name) SELECT i, 'T'||i,
               CASE i%8 WHEN 0 THEN '北京滴滴出行科技有限公司' WHEN 1 THEN '无锡众允机电设备有限公司' WHEN 2 THEN '南京领行汽车服务有限公司'
                        WHEN 3 THEN '山东及时雨汽车科技有限公司灌南分公司' WHEN 4 THEN '绿发酒店管理（北京）有限公司无锡万豪酒店'
                        WHEN 5 THEN '上海大众交通集团股份有限公司' WHEN 6 THEN '中国石化销售股份有限公司北京石油分公司' ELSE '广州顺风车服务有限公司' END
               || '第' || i || '分部' FROM n;
             WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i+1 FROM n WHERE i<500000)
             INSERT INTO invoice_headers(id, e_num, seller_id, issue_date, amount, tax, total, source, inv_type, state, is_pos, risk, remark)
             SELECT i, 'E'||i, (i*7919)%20000+1, '2026-09-01', 100, 9, 109, 1, 1, 1, 1, 1,
               CASE i%6 WHEN 0 THEN '2026年10月出差北京打车费用报销' WHEN 1 THEN '公司车辆加油费用，92号汽油' WHEN 2 THEN '员工住宿费，出差上海三晚'
                        WHEN 3 THEN '会议餐饮服务费，客户接待' WHEN 4 THEN '办公用品采购，打印纸与墨盒' ELSE '设备维修保养费用' END || '批次' || i
             FROM n;",
        )
        .unwrap(); // 触发器会逐行写 FTS，这一步包含建索引时间
        eprintln!("灌入 50 万发票头（含触发器写索引）: {:.1}s", t0.elapsed().as_secs_f64());

        let time = |label: &str, seller: Option<&str>, remark: Option<&str>| {
            let f = InvoiceHeaderFilter {
                seller_name: seller.map(Into::into),
                remark: remark.map(Into::into),
                ..Default::default()
            };
            query_invoice_headers(&c, &f).unwrap(); // 预热
            let mut v: Vec<f64> = (0..5)
                .map(|_| {
                    let t = Instant::now();
                    let n = query_invoice_headers(&c, &f).unwrap().1;
                    let _ = n;
                    t.elapsed().as_secs_f64() * 1000.0
                })
                .collect();
            v.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let (_, total) = query_invoice_headers(&c, &f).unwrap();
            eprintln!("{label:<28} 命中 {total:>7}   中位 {:>7.0} ms", v[2]);
        };
        time("销方 滴滴出行 (trigram)", Some("滴滴出行"), None);
        time("销方 分公司 (trigram,后缀)", Some("分公司"), None);
        time("备注 打车费用 (trigram)", None, Some("打车费用"));
        time("备注 批次499999 (trigram)", None, Some("批次499999"));
        time("销方 滴滴 (2字,LIKE回退)", Some("滴滴"), None);
        time("备注 打车 (2字,LIKE回退)", None, Some("打车"));
    }
}
