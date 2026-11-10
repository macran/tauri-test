// 红字视角与对应关系查询（只读）。
//
// 口径：
//   - 红字视角的范围与红冲核对一致：红字发票（is_pos = 0）且有对应蓝票号码（rev_org_key）。
//   - 红冲结果表里 matched_* 一律是正数；红字发票自己的金额是负数。比较时红票金额取绝对值。
//   - 已匹配 = match_status = 1 且 blue_detail_id 非空的记录之和。核对在额度不足时会写一条
//     blue_detail_id 为空的“异常”记录，存的是没冲掉的部分，它只提供核对结果和备注，不计入已匹配。
//   - 核对结果：红票取其明细里最严重的一个（异常 > 无蓝字发票 > 未核对 > 成功）。
//     没有任何记录 = 未核对。
//   - 对应蓝票号码取红票头的 rev_org_key，蓝票不在库里也有值，只是蓝票开票日期为空。

use std::collections::{HashMap, HashSet};

use rusqlite::{types::Value, Connection, Result as SqlResult};

use super::models::*;
use super::repository::{fts_seller, like, qty_text, s, Line, MATCH_NORMAL, MAX_ROWS, SEP};

/// 严重程度：异常 > 无蓝字发票 > 未核对 > 成功
/// 结果取值：0 未核对 / 1 成功 / 2 无蓝字发票 / 3 红字发票异常
fn severity(r: u8) -> u8 {
    match r {
        3 => 4,
        2 => 3,
        0 => 2,
        _ => 1,
    }
}

#[derive(Default, Clone)]
struct DetailMatch {
    amount: i64,
    tax: i64,
    total: i64,
    blue_lines: Vec<u32>,
    result: Option<u8>,
    note: Option<String>,
}

#[derive(Default)]
struct HeadAgg {
    amount: i64,
    tax: i64,
    total: i64,
    result: u8,
    note: Option<String>,
    seen: bool,
}

struct RedHead {
    id: i64,
    code: Option<String>,
    num: Option<String>,
    e_num: Option<String>,
    seller_tax_no: String,
    seller_name: String,
    issue_date: String,
    inv_type: u8,
    state: u8,
    remark: Option<String>,
    amount: i64,
    tax: i64,
    total: i64,
    notice: Option<String>,
    blue_key: Option<String>,
    blue_date: Option<String>,
}

fn add_date_range(w: &mut Vec<String>, p: &mut Vec<Value>, col: &str, f: &ReversalQueryFilter) {
    if let Some(d) = f.issue_date_from.as_deref().map(str::trim).filter(|d| !d.is_empty()) {
        w.push(format!("{col} >= ?"));
        p.push(Value::Text(d.into()));
    }
    if let Some(d) = f.issue_date_to.as_deref().map(str::trim).filter(|d| !d.is_empty()) {
        w.push(format!("{col} <= ?"));
        p.push(Value::Text(d.into()));
    }
}

pub fn query_red(conn: &Connection, f: &ReversalQueryFilter) -> SqlResult<ReversalQueryResult> {
    // ───── 1. 红字发票头 ─────
    let mut w: Vec<String> = vec![
        "h.is_pos = 0".into(),
        "h.rev_org_key IS NOT NULL AND TRIM(h.rev_org_key) <> ''".into(),
    ];
    let mut p: Vec<Value> = Vec::new();
    like(&mut w, &mut p, "h.inv_unq_key", &f.invoice_number);
    fts_seller(&mut w, &mut p, "h", &f.seller_name);
    like(&mut w, &mut p, "s.tax_no", &f.seller_tax_no);
    add_date_range(&mut w, &mut p, "h.issue_date", f);

    let sql = format!(
        "SELECT h.id, h.code, h.num, h.e_num, s.tax_no, s.name, h.issue_date, h.inv_type, h.state, \
         h.remark, h.amount, h.tax, h.total, h.red_notice_no, h.rev_org_key, bh.issue_date \
         FROM invoice_headers h JOIN sellers s ON s.id = h.seller_id \
         LEFT JOIN invoice_headers bh ON bh.inv_unq_key = h.rev_org_key AND bh.is_pos = 1 \
         WHERE {} ORDER BY h.issue_date DESC, h.id DESC",
        w.join(" AND ")
    );
    let mut stmt = conn.prepare(&sql)?;
    let heads = stmt
        .query_map(rusqlite::params_from_iter(&p), |r| {
            Ok(RedHead {
                id: r.get(0)?,
                code: r.get(1)?,
                num: r.get(2)?,
                e_num: r.get(3)?,
                seller_tax_no: r.get(4)?,
                seller_name: r.get(5)?,
                issue_date: r.get(6)?,
                inv_type: r.get::<_, i64>(7)? as u8,
                state: r.get::<_, i64>(8)? as u8,
                remark: r.get(9)?,
                amount: r.get(10)?,
                tax: r.get(11)?,
                total: r.get(12)?,
                notice: r.get(13)?,
                blue_key: r.get(14)?,
                blue_date: r.get(15)?,
            })
        })?
        .collect::<SqlResult<Vec<_>>>()?;
    if heads.is_empty() {
        return Ok(ReversalQueryResult::default());
    }

    conn.execute_batch(
        "CREATE TEMP TABLE IF NOT EXISTS _rq_r (id INTEGER PRIMARY KEY); DELETE FROM _rq_r;",
    )?;
    {
        let mut ins = conn.prepare("INSERT INTO _rq_r(id) VALUES (?1)")?;
        for h in &heads {
            ins.execute([h.id])?;
        }
    }

    // ───── 2. 红字明细 ─────
    let mut stmt = conn.prepare(
        "SELECT d.id, d.header_id, d.line_no, d.tax_code, d.goods_name, d.spec_model, d.unit, \
         d.quantity, d.unit_price, d.tax_rate, d.amount, d.tax, d.total \
         FROM invoice_details d JOIN _rq_r t ON t.id = d.header_id ORDER BY d.header_id, d.line_no",
    )?;
    let lines = stmt
        .query_map([], |r| {
            Ok(Line {
                id: r.get(0)?,
                header_id: r.get(1)?,
                line_no: r.get(2)?,
                tax_code: r.get(3)?,
                goods_name: r.get(4)?,
                spec_model: r.get(5)?,
                unit: r.get(6)?,
                quantity: r.get(7)?,
                unit_price: r.get(8)?,
                tax_rate: r.get(9)?,
                amount: r.get(10)?,
                tax: r.get(11)?,
                total: r.get(12)?,
                is_discount: false,
            })
        })?
        .collect::<SqlResult<Vec<_>>>()?;

    // ───── 3. 每个红字明细的核对记录 ─────
    let mut matched: HashMap<i64, DetailMatch> = HashMap::new();
    let mut stmt = conn.prepare(
        "SELECT m.red_detail_id, m.blue_detail_id, bd.line_no, m.matched_amount, m.matched_tax, \
         m.matched_total, m.match_result_type, m.match_status, m.match_note \
         FROM reversal_line_matches m \
         JOIN _rq_r t ON t.id = m.red_header_id \
         LEFT JOIN invoice_details bd ON bd.id = m.blue_detail_id \
         ORDER BY m.id",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, Option<i64>>(1)?,
            r.get::<_, Option<i64>>(2)?,
            r.get::<_, i64>(3)?,
            r.get::<_, i64>(4)?,
            r.get::<_, i64>(5)?,
            r.get::<_, i64>(6)? as u8,
            r.get::<_, i64>(7)?,
            r.get::<_, Option<String>>(8)?,
        ))
    })?;
    for row in rows {
        let (did, blue_did, bline, amount, tax, total, result, status, note) = row?;
        let e = matched.entry(did).or_default();
        if blue_did.is_some() && status == MATCH_NORMAL {
            e.amount += amount;
            e.tax += tax;
            e.total += total;
            if let Some(l) = bline {
                if !e.blue_lines.contains(&(l as u32)) {
                    e.blue_lines.push(l as u32);
                }
            }
        }
        // 结果取最严重的一条；备注跟随被选中的那条（严重程度相同则保留先到的有备注的）
        match e.result {
            Some(cur) if severity(result) < severity(cur) => {}
            Some(cur) if severity(result) == severity(cur) => {
                if e.note.is_none() {
                    e.note = note;
                }
            }
            _ => {
                e.result = Some(result);
                e.note = note;
            }
        }
    }

    // ───── 4. 凭证（与蓝字视角同一口径）─────
    let mut vouchers: HashMap<i64, Vec<ReversalVoucher>> = HashMap::new();
    let mut add = |sql: &str| -> SqlResult<()> {
        let mut stmt = conn.prepare(sql)?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                ReversalVoucher { voucher_no: r.get(1)?, posted_date: r.get(2)? },
            ))
        })?;
        for row in rows {
            let (hid, v) = row?;
            vouchers.entry(hid).or_default().push(v);
        }
        Ok(())
    };
    add("SELECT a.header_id, a.voucher_no, a.entry_date FROM ap_invoice_vouchers a \
         JOIN _rq_r t ON t.id = a.header_id ORDER BY a.id")?;
    add("SELECT i.header_id, i.document_no, i.posted_date FROM ic_vouchers i \
         JOIN _rq_r t ON t.id = i.header_id ORDER BY i.id")?;
    add("SELECT b.header_id, b.voucher_no, b.posted_date FROM bank_invoice_vouchers b \
         JOIN _rq_r t ON t.id = b.header_id")?;
    add("SELECT c.header_id, c.voucher_no, c.posted_date FROM certified_list c \
         JOIN _rq_r t ON t.id = c.header_id \
         WHERE c.is_travel_expense = 1 AND c.voucher_no IS NOT NULL")?;
    drop(add);
    conn.execute_batch("DROP TABLE IF EXISTS _rq_r")?;

    let expand_all = matches!(f.expand.as_deref(), Some("ap" | "ic"));
    let vouchers_of = |hid: i64| -> (Vec<Option<ReversalVoucher>>, u32) {
        let v = vouchers.get(&hid).cloned().unwrap_or_default();
        let n = v.len() as u32;
        match (expand_all, v.len()) {
            (_, 0) => (vec![None], 0),
            (true, _) => (v.into_iter().map(Some).collect(), n),
            (false, _) => (vec![v.into_iter().next()], n),
        }
    };

    // ───── 5. 红字明细行，同时累计到红票 ─────
    let head_of: HashMap<i64, &RedHead> = heads.iter().map(|h| (h.id, h)).collect();
    let mut agg: HashMap<i64, HeadAgg> = HashMap::new();
    let mut details: Vec<RedDetailRow> = Vec::new();
    for l in &lines {
        let Some(h) = head_of.get(&l.header_id) else { continue };
        let m = matched.get(&l.id).cloned().unwrap_or_default();
        // 红字金额为负，匹配表里是正数：统一按绝对值
        let (a, t, tot) = (l.amount.abs(), l.tax.abs(), l.total.abs());
        let result = m.result.unwrap_or(0);

        let g = agg.entry(l.header_id).or_default();
        g.amount += m.amount;
        g.tax += m.tax;
        g.total += m.total;
        if !g.seen || severity(result) > severity(g.result) {
            g.result = result;
            g.note = m.note.clone();
            g.seen = true;
        }

        let blue_lines = (!m.blue_lines.is_empty())
            .then(|| m.blue_lines.iter().map(u32::to_string).collect::<Vec<_>>().join(SEP));
        let (vs, vcount) = vouchers_of(l.header_id);
        for v in vs {
            details.push(RedDetailRow {
                detail_id: l.id as u32,
                header_id: l.header_id as u32,
                line_no: l.line_no as u32,
                code: h.code.clone(),
                num: h.num.clone(),
                e_num: h.e_num.clone(),
                seller_tax_no: h.seller_tax_no.clone(),
                seller_name: h.seller_name.clone(),
                issue_date: h.issue_date.clone(),
                tax_code: l.tax_code.clone(),
                goods_name: l.goods_name.clone(),
                spec_model: l.spec_model.clone(),
                unit: l.unit.clone(),
                quantity: l.quantity.clone(),
                unit_price: l.unit_price.clone(),
                tax_rate: l.tax_rate.clone(),
                inv_type: h.inv_type,
                state: h.state,
                remark: h.remark.clone(),
                amount: s(a),
                tax: s(t),
                total: s(tot),
                matched_amount: s(m.amount),
                matched_tax: s(m.tax),
                matched_total: s(m.total),
                unmatched_amount: s(a - m.amount),
                unmatched_tax: s(t - m.tax),
                unmatched_total: s(tot - m.total),
                red_notice_no: h.notice.clone(),
                blue_inv_key: h.blue_key.clone(),
                blue_issue_date: h.blue_date.clone(),
                blue_line_nos: blue_lines.clone(),
                match_result: result,
                match_note: m.note.clone(),
                voucher: v,
                voucher_count: vcount,
            });
        }
    }

    // ───── 6. 红字汇总行；核对结果筛选作用在红票上，明细随红票走 ─────
    let want = f.match_result.as_ref().filter(|v| !v.is_empty());
    let keep = |r: u8| want.map_or(true, |v| v.contains(&r));
    let mut summary: Vec<RedSummaryRow> = Vec::new();
    let mut kept: HashSet<i64> = HashSet::new();
    for h in &heads {
        let g = agg.remove(&h.id).unwrap_or_default();
        if !keep(g.result) {
            continue;
        }
        kept.insert(h.id);
        let (a, t, tot) = (h.amount.abs(), h.tax.abs(), h.total.abs());
        let (vs, vcount) = vouchers_of(h.id);
        for v in vs {
            summary.push(RedSummaryRow {
                header_id: h.id as u32,
                code: h.code.clone(),
                num: h.num.clone(),
                e_num: h.e_num.clone(),
                seller_tax_no: h.seller_tax_no.clone(),
                seller_name: h.seller_name.clone(),
                issue_date: h.issue_date.clone(),
                inv_type: h.inv_type,
                state: h.state,
                remark: h.remark.clone(),
                amount: s(a),
                tax: s(t),
                total: s(tot),
                matched_amount: s(g.amount),
                matched_tax: s(g.tax),
                matched_total: s(g.total),
                unmatched_amount: s(a - g.amount),
                unmatched_tax: s(t - g.tax),
                unmatched_total: s(tot - g.total),
                red_notice_no: h.notice.clone(),
                blue_inv_key: h.blue_key.clone(),
                blue_issue_date: h.blue_date.clone(),
                match_result: g.result,
                match_note: g.note.clone(),
                voucher: v,
                voucher_count: vcount,
            });
        }
    }
    details.retain(|d| kept.contains(&(d.header_id as i64)));
    let summary_total = kept.len() as u32;
    let detail_total = details.len() as u32;
    summary.truncate(MAX_ROWS);
    details.truncate(MAX_ROWS);

    Ok(ReversalQueryResult {
        red_summary: summary,
        red_details: details,
        summary_total,
        detail_total,
        ..Default::default()
    })
}

/// 对应关系：一行 = 一条红冲匹配记录（红字明细 ↔ 蓝字明细）。
/// 同一批记录，蓝字视角读作“这批蓝票被谁冲了”，红字视角读作“这批红票冲了谁”，列一样，区别只是
/// 筛选范围跟着主表走，保证汇总表里的累计数点开就能在这里找到对应记录：
///   - 蓝字视角：日期、发票状态按蓝票过滤（蓝票不在库的记录不会出现）
///   - 红字视角：日期、核对结果按红票过滤
/// 发票号码在红票或蓝票任一边命中即可。
pub fn query_links(conn: &Connection, f: &ReversalQueryFilter) -> SqlResult<ReversalQueryResult> {
    let red = f.view.as_deref() == Some("red");
    let mut w: Vec<String> = Vec::new();
    let mut p: Vec<Value> = Vec::new();
    if let Some(t) = f.invoice_number.as_deref().map(str::trim).filter(|t| !t.is_empty()) {
        w.push("(rh.inv_unq_key LIKE ? OR bh.inv_unq_key LIKE ? OR rh.rev_org_key LIKE ?)".into());
        for _ in 0..3 {
            p.push(Value::Text(format!("%{t}%")));
        }
    }
    fts_seller(&mut w, &mut p, "rh", &f.seller_name);
    like(&mut w, &mut p, "s.tax_no", &f.seller_tax_no);
    if red {
        add_date_range(&mut w, &mut p, "rh.issue_date", f);
        if let Some(v) = f.match_result.as_ref().filter(|v| !v.is_empty()) {
            // 对应关系里只有已核对的记录，0（未核对）在这里不会出现
            w.push(format!("m.match_result_type IN ({})", vec!["?"; v.len()].join(",")));
            p.extend(v.iter().map(|x| Value::Integer(*x as i64)));
        }
    } else {
        add_date_range(&mut w, &mut p, "bh.issue_date", f);
        if let Some(v) = f.state.as_ref().filter(|v| !v.is_empty()) {
            w.push(format!("bh.state IN ({})", vec!["?"; v.len()].join(",")));
            p.extend(v.iter().map(|x| Value::Integer(*x as i64)));
        }
        // 蓝字视角只看冲到了蓝票上的记录；蓝票不在库的占位记录属于红字视角
        w.push("m.blue_header_id IS NOT NULL".into());
    }
    let where_sql = if w.is_empty() { String::new() } else { format!(" WHERE {}", w.join(" AND ")) };

    let order = if red {
        "rh.issue_date DESC, m.red_header_id DESC, rd.line_no, m.id"
    } else {
        "bh.issue_date DESC, m.blue_header_id DESC, bd.line_no, m.id"
    };
    // 红字视角下，无蓝字发票的占位记录（match_status = 2）也是对应关系的一部分，一并展示
    let sql = format!(
        "SELECT m.id, m.red_header_id, rh.code, rh.num, rh.e_num, rd.line_no, rh.issue_date, \
         rh.red_notice_no, bh.code, bh.num, bh.e_num, rh.rev_org_key, bd.line_no, bh.issue_date, \
         s.name, m.matched_quantity, m.matched_amount, m.matched_tax, m.matched_total, \
         m.match_result_type, m.match_note \
         FROM reversal_line_matches m \
         JOIN invoice_headers rh ON rh.id = m.red_header_id \
         JOIN invoice_details rd ON rd.id = m.red_detail_id \
         JOIN sellers s ON s.id = rh.seller_id \
         LEFT JOIN invoice_headers bh ON bh.id = m.blue_header_id \
         LEFT JOIN invoice_details bd ON bd.id = m.blue_detail_id\
         {where_sql} ORDER BY {order}"
    );
    let total: i64 =
        conn.query_row(&format!("SELECT COUNT(*) FROM ({sql})"), rusqlite::params_from_iter(&p), |r| r.get(0))?;
    let mut stmt = conn.prepare(&format!("{sql} LIMIT {MAX_ROWS}"))?;
    let links = stmt
        .query_map(rusqlite::params_from_iter(&p), |r| {
            Ok(ReversalLinkRow {
                id: r.get::<_, i64>(0)? as u32,
                red_header_id: r.get::<_, i64>(1)? as u32,
                red_code: r.get(2)?,
                red_num: r.get(3)?,
                red_e_num: r.get(4)?,
                red_line_no: r.get::<_, i64>(5)? as u32,
                red_issue_date: r.get(6)?,
                red_notice_no: r.get(7)?,
                blue_code: r.get(8)?,
                blue_num: r.get(9)?,
                blue_e_num: r.get(10)?,
                blue_inv_key: r.get(11)?,
                blue_line_no: r.get::<_, Option<i64>>(12)?.map(|v| v as u32),
                blue_issue_date: r.get(13)?,
                seller_name: r.get(14)?,
                matched_quantity: r.get::<_, Option<f64>>(15)?.map(qty_text),
                matched_amount: s(r.get(16)?),
                matched_tax: s(r.get(17)?),
                matched_total: s(r.get(18)?),
                match_result: r.get::<_, i64>(19)? as u8,
                match_note: r.get(20)?,
            })
        })?
        .collect::<SqlResult<Vec<_>>>()?;
    Ok(ReversalQueryResult { links, link_total: total as u32, ..Default::default() })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conn() -> Connection {
        let mut c = Connection::open_in_memory().unwrap();
        // init_db 建 FTS5 表需要自定义分词器 simple
        unsafe {
            c.load_extension_enable().unwrap();
            let dll = concat!(env!("CARGO_MANIFEST_DIR"), "/resources/simple.dll");
            c.load_extension(dll, None::<&str>).ok();
        }
        crate::db::schema::init_db(&mut c).unwrap();
        // 蓝票 B1 两行；R1 全额冲成功；R2 额度不足（冲 1000，缺 500 -> 异常记录）；
        // R3 对应蓝票不在库里（无蓝字发票占位）；R4 还没核对
        c.execute_batch(
            "INSERT INTO sellers(id, tax_no, name) VALUES (1,'T1','甲');
             INSERT INTO invoice_headers(id,e_num,seller_id,issue_date,amount,tax,total,source,inv_type,state,is_pos,risk,red_notice_no,rev_org_key)
               VALUES (1,'B1',1,'2026-09-01',10000,900,10900,1,1,4,1,1,NULL,NULL),
                      (2,'R1',1,'2026-09-10',-3000,-270,-3270,1,1,1,0,1,'N-1','B1'),
                      (3,'R2',1,'2026-09-11',-1500,-135,-1635,1,1,1,0,1,'N-2','B1'),
                      (4,'R3',1,'2026-09-12',-1000,-90,-1090,1,1,1,0,1,'N-3','GONE'),
                      (5,'R4',1,'2026-09-13',-500,-45,-545,1,1,1,0,1,'N-4','B1');
             INSERT INTO invoice_details(id,header_id,line_no,goods_name,quantity,amount,tax_rate,tax,total)
               VALUES (1,1,1,'a','10',6000,'9%',540,6540),
                      (2,1,2,'b','5',4000,'9%',360,4360),
                      (3,2,1,'a','-3',-3000,'9%',-270,-3270),
                      (4,3,1,'b','-1',-1500,'9%',-135,-1635),
                      (5,4,1,'c','-1',-1000,'9%',-90,-1090),
                      (6,5,1,'a','-1',-500,'9%',-45,-545);
             INSERT INTO reversal_line_matches(red_header_id,red_detail_id,blue_header_id,blue_detail_id,matched_quantity,matched_amount,matched_tax,matched_total,match_result_type,match_status,match_note,match_batch_no)
               VALUES (2,3,1,1,3,3000,270,3270,1,1,NULL,1),
                      (3,4,1,2,1,1000,90,1090,1,1,NULL,1),
                      (3,4,1,NULL,NULL,500,45,545,3,1,'额度不足',1),
                      (4,5,NULL,NULL,NULL,0,0,0,2,2,'找不到蓝票',1);",
        )
        .unwrap();
        c
    }

    fn red(c: &Connection, f: ReversalQueryFilter) -> ReversalQueryResult {
        query_red(c, &ReversalQueryFilter { view: Some("red".into()), ..f }).unwrap()
    }

    #[test]
    fn red_view_results_and_unmatched() {
        let c = conn();
        let r = red(&c, ReversalQueryFilter::default());
        assert_eq!(r.red_summary.len(), 4);
        let by = |e: &str| r.red_summary.iter().find(|x| x.e_num.as_deref() == Some(e)).unwrap();

        // 成功：全额冲掉，余额 0，金额取绝对值
        let r1 = by("R1");
        assert_eq!((r1.match_result, r1.total.as_str(), r1.matched_total.as_str(), r1.unmatched_total.as_str()), (1, "3270", "3270", "0"));
        assert_eq!(r1.blue_inv_key.as_deref(), Some("B1"));
        assert_eq!(r1.blue_issue_date.as_deref(), Some("2026-09-01"));

        // 额度不足：只冲了 1090，余额 545；空 blue_detail 的异常记录只给结果，不计入已匹配
        let r2 = by("R2");
        assert_eq!((r2.match_result, r2.matched_total.as_str(), r2.unmatched_total.as_str()), (3, "1090", "545"));
        assert_eq!(r2.match_note.as_deref(), Some("额度不足"));

        // 蓝票不在库：号码仍显示，开票日期为空
        let r3 = by("R3");
        assert_eq!((r3.match_result, r3.blue_inv_key.as_deref(), r3.blue_issue_date.as_deref()), (2, Some("GONE"), None));

        // 没核对过
        let r4 = by("R4");
        assert_eq!((r4.match_result, r4.matched_total.as_str(), r4.unmatched_total.as_str()), (0, "0", "545"));
    }

    #[test]
    fn red_view_detail_lines_and_filter() {
        let c = conn();
        let r = red(&c, ReversalQueryFilter::default());
        let d = r.red_details.iter().find(|x| x.e_num.as_deref() == Some("R2")).unwrap();
        assert_eq!(d.blue_line_nos.as_deref(), Some("2"));

        // 只看未核对 / 异常
        let r = red(&c, ReversalQueryFilter { match_result: Some(vec![0, 3]), ..Default::default() });
        assert_eq!(r.red_summary.len(), 2);
        assert_eq!(r.red_details.len(), 2);
        assert_eq!(r.summary_total, 2);
    }

    /// 销方名称检索：query_links 的发票头别名是 rh（红票），别名写错只会在运行时才报错。
    /// 「乙贸易」3 个字符走 trigram 索引，「甲」1 个字符回退 LIKE，两条路径都要覆盖。
    #[test]
    fn seller_name_search_uses_fts_in_red_and_links_views() {
        let c = conn();
        // 再加一张别家销方的红票，让「命中 / 不命中」有区别
        c.execute_batch(
            "INSERT INTO sellers(id, tax_no, name) VALUES (2,'T2','乙贸易有限公司');
             INSERT INTO invoice_headers(id,e_num,seller_id,issue_date,amount,tax,total,source,inv_type,state,is_pos,risk,red_notice_no,rev_org_key)
               VALUES (6,'R9',2,'2026-09-14',-100,-9,-109,1,1,1,0,1,'N-9','B9');
             INSERT INTO invoice_details(id,header_id,line_no,goods_name,quantity,amount,tax_rate,tax,total)
               VALUES (7,6,1,'z','-1',-100,'9%',-9,-109);
             INSERT INTO reversal_line_matches(red_header_id,red_detail_id,blue_header_id,blue_detail_id,matched_quantity,matched_amount,matched_tax,matched_total,match_result_type,match_status,match_note,match_batch_no)
               VALUES (6,7,NULL,NULL,NULL,0,0,0,3,2,'无蓝票',1);")
        .unwrap();
        let by = |name: &str| ReversalQueryFilter {
            view: Some("red".into()),
            seller_name: Some(name.into()),
            ..Default::default()
        };
        // 红字视角的明细与对应关系，都只命中该销方
        let own = query_links(&c, &by("乙贸易")).unwrap();
        assert_eq!(own.links.len(), 1);
        assert_eq!(own.links[0].red_e_num.as_deref(), Some("R9"));
        assert_eq!(query_links(&c, &by("甲")).unwrap().links.len(), 4);
        assert!(query_links(&c, &by("不存在")).unwrap().links.is_empty());
        // 空串不加条件（jieba_query('') 会报语法错误）
        assert_eq!(query_links(&c, &by("")).unwrap().links.len(), 5);
        // query_red 同一过滤也能执行
        assert!(query_red(&c, &by("乙贸易")).is_ok());
    }

    #[test]
    fn links_one_row_per_match_record() {
        let c = conn();
        // 红字视角：4 条匹配记录（含异常记录和蓝票不在库的占位记录）；未核对的 R4 没有记录
        let red = ReversalQueryFilter { view: Some("red".into()), ..Default::default() };
        let l = query_links(&c, &red).unwrap();
        assert_eq!((l.links.len(), l.link_total), (4, 4));
        let gone = l.links.iter().find(|x| x.red_e_num.as_deref() == Some("R3")).unwrap();
        assert_eq!((gone.blue_inv_key.as_deref(), gone.blue_line_no, gone.match_result), (Some("GONE"), None, 2));
        let ok = l.links.iter().find(|x| x.red_e_num.as_deref() == Some("R1")).unwrap();
        assert_eq!((ok.blue_e_num.as_deref(), ok.blue_line_no, ok.matched_total.as_str()), (Some("B1"), Some(1), "3270"));

        // 发票号码在蓝票一侧命中也能查到：B1 对应 R1（1 条）和 R2（2 条，含异常记录），R3 的蓝票不是 B1
        let l = query_links(&c, &ReversalQueryFilter { invoice_number: Some("B1".into()), ..Default::default() }).unwrap();
        assert_eq!(l.links.len(), 3);
    }

    #[test]
    fn links_follow_the_view_scope() {
        let c = conn();
        // 蓝票 B1 开在 09-01，红票 09-10 ~ 09-13。日期只框住蓝票：
        let blue = ReversalQueryFilter {
            issue_date_from: Some("2026-09-01".into()),
            issue_date_to: Some("2026-09-05".into()),
            ..Default::default()
        };
        let l = query_links(&c, &blue).unwrap();
        // 汇总表说 B1 被冲了，这里必须能找到冲它的记录：R1 的 1 条 + R2 的 2 条（含异常记录），R3 蓝票不在库不出现
        assert_eq!(l.links.len(), 3);
        assert!(l.links.iter().all(|x| x.blue_issue_date.as_deref() == Some("2026-09-01")));

        // 同样的日期换成红字视角：红票都不在区间内，没有记录
        let l = query_links(&c, &ReversalQueryFilter { view: Some("red".into()), ..blue.clone() }).unwrap();
        assert_eq!(l.links.len(), 0);

        // 红字视角框住红票：R3（蓝票不在库）的占位记录也在
        let red = ReversalQueryFilter {
            view: Some("red".into()),
            issue_date_from: Some("2026-09-12".into()),
            issue_date_to: Some("2026-09-12".into()),
            ..Default::default()
        };
        let l = query_links(&c, &red).unwrap();
        assert_eq!(l.links.len(), 1);
        assert_eq!(l.links[0].blue_inv_key.as_deref(), Some("GONE"));

        // 蓝字视角的发票状态也作用到对应关系：B1 是 4（已红冲-部分）
        let l = query_links(&c, &ReversalQueryFilter { state: Some(vec![1]), ..Default::default() }).unwrap();
        assert_eq!(l.links.len(), 0);
    }
}
