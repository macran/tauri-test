// 红冲查询（只读）。
//
// 口径：
//   - 只查蓝字发票（is_pos = 1）。红字发票本身不作为行出现，只通过 reversal_line_matches 体现为“累计冲红”。
//   - 累计冲红只算 match_status = 1（正常）且 blue_detail_id 非空的记录；“无蓝字发票”占位记录不计。
//   - 累计冲红先在子查询里按蓝字明细聚合，再合并到明细/发票头，不把红冲表直接 JOIN 到主表后 SUM，
//     避免一个明细被多条红冲记录或多条凭证 JOIN 后重复累加。
//   - 蓝字折扣行单独成行（只标记，不合并），明细条数与发票查询一致。红冲记录的 blue_detail_id
//     指向被折扣行，所以折扣行自己的累计冲红为 0；被折扣行的剩余额不扣折扣，两行合起来才是核对口径。
//   - 凭证（AP / IC / Bank / 差旅）一张发票只会落在一种里，但 AP、IC 可能有多条。
//     默认一行只带第一条凭证和凭证总数；expand = ap / ic 时才展开成多行，一次只展开一个维度，
//     不会出现多个一对多维度相乘的行数膨胀。

use std::collections::HashMap;

use rusqlite::{types::Value, Connection, Result as SqlResult};

use super::models::*;

pub(super) const MAX_ROWS: usize = 50_000;
pub(super) const MATCH_NORMAL: i64 = 1;
pub(super) const SEP: &str = "、";

#[derive(Default, Clone)]
struct Reversed {
    quantity: f64,
    has_quantity: bool,
    amount: i64,
    tax: i64,
    total: i64,
    notices: Vec<String>,
    red_nos: Vec<String>,
}

struct Head {
    id: i64,
    code: Option<String>,
    num: Option<String>,
    e_num: Option<String>,
    seller_tax_no: String,
    seller_name: String,
    issue_date: String,
    inv_type: u8,
    state: u8,
    is_pos: u8,
    remark: Option<String>,
    amount: i64,
    tax: i64,
    total: i64,
}

pub(super) struct Line {
    pub(super) id: i64,
    pub(super) header_id: i64,
    pub(super) line_no: i64,
    pub(super) tax_code: Option<String>,
    pub(super) goods_name: String,
    pub(super) spec_model: Option<String>,
    pub(super) unit: Option<String>,
    pub(super) quantity: Option<String>,
    pub(super) unit_price: Option<String>,
    pub(super) tax_rate: Option<String>,
    pub(super) amount: i64,
    pub(super) tax: i64,
    pub(super) total: i64,
    /// 本行是折扣行（紧跟在同名同规格行之后、金额税额合计都 <= 0）
    pub(super) is_discount: bool,
}

pub(super) fn s(v: i64) -> String {
    v.to_string()
}

pub(super) fn qty_text(v: f64) -> String {
    // 数量是文本字段，展示时去掉多余小数 0
    let t = format!("{v:.8}");
    t.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// 销方名称的检索：≥ 3 个字符走 invoice_headers_fts（trigram），否则回退 `s.name LIKE`。
/// `header` 是发票头的表别名。规则与综合查询共用 query::text_search。
pub(super) fn fts_seller(w: &mut Vec<String>, p: &mut Vec<Value>, header: &str, v: &Option<String>) {
    if let Some((clause, param)) = crate::features::query::repository::text_search(header, "seller_name", "s.name", v.as_deref()) {
        w.push(clause);
        p.push(param);
    }
}

pub(super) fn like(w: &mut Vec<String>, p: &mut Vec<Value>, col: &str, v: &Option<String>) {
    if let Some(t) = v.as_deref().map(str::trim).filter(|t| !t.is_empty()) {
        w.push(format!("{col} LIKE ?"));
        p.push(Value::Text(format!("%{t}%")));
    }
}

/// 折扣行判定的输入：一个发票明细的最少信息
pub struct DiscountInput<'a> {
    pub header_id: i64,
    pub goods_name: &'a str,
    pub spec_model: Option<&'a str>,
    pub amount: i64,
    pub tax: i64,
    pub total: i64,
}

/// 折扣行判定，与红冲核对一致：与上一行同名同规格，且金额/税额/价税合计都 <= 0。
/// `rows` 必须是**整张发票**的明细，并已按 (header_id, line_no) 排好序；返回值与输入等长。
/// 已被判为折扣行的行不会再去吸附下一行（与核对里“一个折扣行并入一个被折扣行”一致）。
/// 发票明细查询和红冲查询共用这一份，保证两个页面的折扣行数一致。
pub fn discount_flags(rows: &[DiscountInput]) -> Vec<bool> {
    let mut flags = vec![false; rows.len()];
    for i in 1..rows.len() {
        let (prev, cur) = (&rows[i - 1], &rows[i]);
        flags[i] = prev.header_id == cur.header_id
            && !flags[i - 1]
            && prev.goods_name.trim() == cur.goods_name.trim()
            && prev.spec_model.unwrap_or("").trim() == cur.spec_model.unwrap_or("").trim()
            && cur.amount <= 0
            && cur.tax <= 0
            && cur.total <= 0;
    }
    flags
}

/// 标记折扣行，只标记不合并：查询页的明细条数要和发票查询一致，用户才能对数。
fn mark_discounts(mut lines: Vec<Line>) -> Vec<Line> {
    lines.sort_by_key(|l| (l.header_id, l.line_no));
    let flags = discount_flags(
        &lines
            .iter()
            .map(|l| DiscountInput {
                header_id: l.header_id,
                goods_name: &l.goods_name,
                spec_model: l.spec_model.as_deref(),
                amount: l.amount,
                tax: l.tax,
                total: l.total,
            })
            .collect::<Vec<_>>(),
    );
    for (l, f) in lines.iter_mut().zip(flags) {
        l.is_discount = f;
    }
    lines
}

pub fn query_reversal(
    conn: &Connection,
    f: &ReversalQueryFilter,
) -> SqlResult<ReversalQueryResult> {
    // ───── 1. 蓝字发票头 ─────
    let mut w: Vec<String> = vec!["h.is_pos = 1".into()];
    let mut p: Vec<Value> = Vec::new();
    like(&mut w, &mut p, "h.inv_unq_key", &f.invoice_number);
    fts_seller(&mut w, &mut p, "h", &f.seller_name);
    like(&mut w, &mut p, "s.tax_no", &f.seller_tax_no);
    if let Some(d) = f.issue_date_from.as_deref().map(str::trim).filter(|d| !d.is_empty()) {
        w.push("h.issue_date >= ?".into());
        p.push(Value::Text(d.into()));
    }
    if let Some(d) = f.issue_date_to.as_deref().map(str::trim).filter(|d| !d.is_empty()) {
        w.push("h.issue_date <= ?".into());
        p.push(Value::Text(d.into()));
    }
    if let Some(v) = f.state.as_ref().filter(|v| !v.is_empty()) {
        w.push(format!("h.state IN ({})", vec!["?"; v.len()].join(",")));
        p.extend(v.iter().map(|x| Value::Integer(*x as i64)));
    }
    let where_sql = w.join(" AND ");

    let head_sql = format!(
        "SELECT h.id, s.tax_no, s.name, h.issue_date, h.inv_type, h.state, h.is_pos, h.remark, \
         h.amount, h.tax, h.total, h.code, h.num, h.e_num \
         FROM invoice_headers h JOIN sellers s ON s.id = h.seller_id \
         WHERE {where_sql} ORDER BY h.issue_date DESC, h.id DESC"
    );
    let mut stmt = conn.prepare(&head_sql)?;
    let all_heads = stmt
        .query_map(rusqlite::params_from_iter(&p), |r| {
            Ok(Head {
                id: r.get(0)?,
                seller_tax_no: r.get(1)?,
                seller_name: r.get(2)?,
                issue_date: r.get(3)?,
                inv_type: r.get::<_, i64>(4)? as u8,
                state: r.get::<_, i64>(5)? as u8,
                is_pos: r.get::<_, i64>(6)? as u8,
                remark: r.get(7)?,
                amount: r.get(8)?,
                tax: r.get(9)?,
                total: r.get(10)?,
                code: r.get(11)?,
                num: r.get(12)?,
                e_num: r.get(13)?,
            })
        })?
        .collect::<SqlResult<Vec<_>>>()?;
    let summary_total = all_heads.len() as u32;
    // 按发票头截断，明细随发票头走，保证两张表对得上
    let heads: Vec<Head> = all_heads.into_iter().take(MAX_ROWS).collect();
    if heads.is_empty() {
        return Ok(ReversalQueryResult { summary_total, ..Default::default() });
    }

    // 用临时表承载入选的发票 id，后面的子查询都 JOIN 它，避免超长 IN 列表
    conn.execute_batch(
        "CREATE TEMP TABLE IF NOT EXISTS _rq_h (id INTEGER PRIMARY KEY); DELETE FROM _rq_h;",
    )?;
    {
        let mut ins = conn.prepare("INSERT INTO _rq_h(id) VALUES (?1)")?;
        for h in &heads {
            ins.execute([h.id])?;
        }
    }

    // ───── 2. 明细（折扣行单独成行，仅做标记）─────
    let mut stmt = conn.prepare(
        "SELECT d.id, d.header_id, d.line_no, d.tax_code, d.goods_name, d.spec_model, d.unit, \
         d.quantity, d.unit_price, d.tax_rate, d.amount, d.tax, d.total \
         FROM invoice_details d JOIN _rq_h t ON t.id = d.header_id",
    )?;
    let lines = mark_discounts(
        stmt.query_map([], |r| {
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
        .collect::<SqlResult<Vec<_>>>()?,
    );

    // ───── 3. 累计冲红：按蓝字明细聚合（子查询里完成，不与明细/凭证相乘）─────
    // 同时取红字信息表编号。一条红冲记录的 red_header_id 对应唯一红票头。
    let mut by_detail: HashMap<i64, Reversed> = HashMap::new();
    let mut by_header: HashMap<i64, Reversed> = HashMap::new();
    let mut stmt = conn.prepare(
        "SELECT m.blue_header_id, m.blue_detail_id, m.matched_quantity, m.matched_amount, \
         m.matched_tax, m.matched_total, rh.red_notice_no, rh.inv_unq_key \
         FROM reversal_line_matches m \
         JOIN _rq_h t ON t.id = m.blue_header_id \
         JOIN invoice_headers rh ON rh.id = m.red_header_id \
         WHERE m.match_status = ?1 AND m.blue_detail_id IS NOT NULL ORDER BY m.id",
    )?;
    let rows = stmt.query_map([MATCH_NORMAL], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, i64>(1)?,
            r.get::<_, Option<f64>>(2)?,
            r.get::<_, i64>(3)?,
            r.get::<_, i64>(4)?,
            r.get::<_, i64>(5)?,
            r.get::<_, Option<String>>(6)?,
            r.get::<_, String>(7)?,
        ))
    })?;
    for row in rows {
        let (hid, did, qty, amount, tax, total, notice, red_no) = row?;
        for rev in [by_detail.entry(did).or_default(), by_header.entry(hid).or_default()] {
            if let Some(q) = qty {
                rev.quantity += q;
                rev.has_quantity = true;
            }
            rev.amount += amount;
            rev.tax += tax;
            rev.total += total;
            if !rev.red_nos.contains(&red_no) {
                rev.red_nos.push(red_no.clone());
            }
            if let Some(n) = notice.as_deref().map(str::trim).filter(|n| !n.is_empty()) {
                if !rev.notices.iter().any(|x| x == n) {
                    rev.notices.push(n.to_string());
                }
            }
        }
    }

    // ───── 4. 凭证：AP / IC 可能多条，Bank / 差旅一对一 ─────
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
    // 先 AP / IC（可多条，按 id 保证稳定顺序），再 Bank、差旅
    add("SELECT a.header_id, a.voucher_no, a.entry_date FROM ap_invoice_vouchers a \
         JOIN _rq_h t ON t.id = a.header_id ORDER BY a.id")?;
    add("SELECT i.header_id, i.document_no, i.posted_date FROM ic_vouchers i \
         JOIN _rq_h t ON t.id = i.header_id ORDER BY i.id")?;
    add("SELECT b.header_id, b.voucher_no, b.posted_date FROM bank_invoice_vouchers b \
         JOIN _rq_h t ON t.id = b.header_id")?;
    add("SELECT c.header_id, c.voucher_no, c.posted_date FROM certified_list c \
         JOIN _rq_h t ON t.id = c.header_id \
         WHERE c.is_travel_expense = 1 AND c.voucher_no IS NOT NULL")?;
    drop(add);
    conn.execute_batch("DROP TABLE IF EXISTS _rq_h")?;

    // 凭证展开：只有 expand 指定时才按多条展开，且只展开一个维度
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

    let notices = |r: Option<&Reversed>| -> Option<String> {
        r.filter(|r| !r.notices.is_empty()).map(|r| r.notices.join(SEP))
    };
    let red_nos = |r: Option<&Reversed>| -> Option<String> {
        r.filter(|r| !r.red_nos.is_empty()).map(|r| r.red_nos.join(SEP))
    };
    let rev0 = Reversed::default();

    // ───── 5. 汇总表 ─────
    let mut summary = Vec::new();
    for h in &heads {
        let rev = by_header.get(&h.id).unwrap_or(&rev0);
        let (vs, vcount) = vouchers_of(h.id);
        for v in vs {
            summary.push(ReversalSummaryRow {
                header_id: h.id as u32,
                code: h.code.clone(),
                num: h.num.clone(),
                e_num: h.e_num.clone(),
                seller_tax_no: h.seller_tax_no.clone(),
                seller_name: h.seller_name.clone(),
                issue_date: h.issue_date.clone(),
                inv_type: h.inv_type,
                state: h.state,
                is_pos: h.is_pos,
                remark: h.remark.clone(),
                amount: s(h.amount),
                tax: s(h.tax),
                total: s(h.total),
                reversed_amount: s(rev.amount),
                reversed_tax: s(rev.tax),
                reversed_total: s(rev.total),
                remain_amount: s(h.amount - rev.amount),
                remain_tax: s(h.tax - rev.tax),
                remain_total: s(h.total - rev.total),
                red_notice_nos: notices(by_header.get(&h.id)),
                red_invoice_nos: red_nos(by_header.get(&h.id)),
                voucher: v,
                voucher_count: vcount,
            });
        }
    }

    // ───── 6. 明细表 ─────
    let head_of: HashMap<i64, &Head> = heads.iter().map(|h| (h.id, h)).collect();
    let mut details = Vec::new();
    // 折扣行统计：按明细去重，在凭证展开之前算
    let mut discount = DiscountStats::default();
    let (mut d_amount, mut d_tax, mut d_total) = (0_i64, 0_i64, 0_i64);
    for l in lines.iter().filter(|l| l.is_discount) {
        discount.count += 1;
        d_amount += l.amount;
        d_tax += l.tax;
        d_total += l.total;
    }
    discount.amount = s(d_amount);
    discount.tax = s(d_tax);
    discount.total = s(d_total);

    for l in &lines {
        let Some(h) = head_of.get(&l.header_id) else { continue };
        let rev = by_detail.get(&l.id).unwrap_or(&rev0);
        let qty: Option<f64> = l.quantity.as_deref().and_then(|q| q.trim().parse().ok());
        let (vs, vcount) = vouchers_of(l.header_id);
        for v in vs {
            details.push(ReversalDetailRow {
                detail_id: l.id as u32,
                is_discount: l.is_discount,
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
                is_pos: h.is_pos,
                remark: h.remark.clone(),
                amount: s(l.amount),
                tax: s(l.tax),
                total: s(l.total),
                reversed_quantity: rev.has_quantity.then(|| qty_text(rev.quantity)),
                reversed_amount: s(rev.amount),
                reversed_tax: s(rev.tax),
                reversed_total: s(rev.total),
                remain_quantity: qty.map(|q| qty_text(q - rev.quantity)),
                remain_amount: s(l.amount - rev.amount),
                remain_tax: s(l.tax - rev.tax),
                remain_total: s(l.total - rev.total),
                red_notice_nos: notices(by_detail.get(&l.id)),
                red_invoice_nos: red_nos(by_detail.get(&l.id)),
                voucher: v,
                voucher_count: vcount,
            });
        }
    }
    let detail_total = details.len() as u32;
    // 汇总表按发票头已截断；明细表同样限制总行数
    details.truncate(MAX_ROWS);

    Ok(ReversalQueryResult { summary, details, summary_total, detail_total, discount, ..Default::default() })
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
        c.execute_batch(
            "INSERT INTO sellers(id, tax_no, name) VALUES (1,'T1','甲');
             INSERT INTO invoice_headers(id,e_num,seller_id,issue_date,amount,tax,total,source,inv_type,state,is_pos,risk,red_notice_no)
               VALUES (1,'B1',1,'2026-09-01',10000,900,10900,1,1,4,1,1,NULL),
                      (2,'R1',1,'2026-09-10',-3000,-270,-3270,1,1,1,0,1,'N-001'),
                      (3,'R2',1,'2026-09-11',-1000,-90,-1090,1,1,1,0,1,'N-002');
             INSERT INTO invoice_details(id,header_id,line_no,goods_name,quantity,amount,tax_rate,tax,total)
               VALUES (1,1,1,'a','10',6000,'9%',540,6540),
                      (2,1,2,'b','5',4000,'9%',360,4360),
                      (3,2,1,'a','-3',-3000,'9%',-270,-3270),
                      (4,3,1,'b','-1',-1000,'9%',-90,-1090);
             INSERT INTO reversal_line_matches(red_header_id,red_detail_id,blue_header_id,blue_detail_id,matched_quantity,matched_amount,matched_tax,matched_total,match_result_type,match_status,match_batch_no)
               VALUES (2,3,1,1,3,3000,270,3270,1,1,1),
                      (3,4,1,2,1,1000,90,1090,1,1,1);
             INSERT INTO ic_vouchers(header_id,legal_entity,document_no,posted_date,received_date,preparer)
               VALUES (1,'E','IC1','2026-09-20','2026-09-20','x'),(1,'E','IC2','2026-09-21','2026-09-21','x');",
        )
        .unwrap();
        c
    }

    #[test]
    fn accumulates_without_multiplying_by_vouchers() {
        let c = conn();
        // 不展开：一张发票一行，累计冲红不被两条 IC 凭证翻倍
        let r = query_reversal(&c, &ReversalQueryFilter::default()).unwrap();
        assert_eq!(r.summary.len(), 1);
        let s = &r.summary[0];
        assert_eq!((s.reversed_total.as_str(), s.remain_total.as_str()), ("4360", "6540"));
        assert_eq!(s.red_notice_nos.as_deref(), Some("N-001、N-002"));
        assert_eq!(s.voucher_count, 2);
        assert_eq!(r.details.len(), 2);
        assert_eq!(r.details[0].remain_quantity.as_deref(), Some("7"));

        // 展开 IC：行数翻倍，但每行累计数不变
        let f = ReversalQueryFilter { expand: Some("ic".into()), ..Default::default() };
        let r = query_reversal(&c, &f).unwrap();
        assert_eq!((r.summary.len(), r.details.len()), (2, 4));
        assert!(r.summary.iter().all(|x| x.reversed_total == "4360"));
    }

    #[test]
    fn state_filter() {
        let c = conn();
        // 种子里的蓝票状态是 4（已红冲-部分）
        let f = ReversalQueryFilter { state: Some(vec![1, 2]), ..Default::default() };
        assert_eq!(query_reversal(&c, &f).unwrap().summary.len(), 0);
        let f = ReversalQueryFilter { state: Some(vec![3, 4]), ..Default::default() };
        let r = query_reversal(&c, &f).unwrap();
        assert_eq!(r.summary.len(), 1);
        assert_eq!(r.summary[0].e_num.as_deref(), Some("B1"));
        assert_eq!(r.details[0].e_num.as_deref(), Some("B1"));
    }

    #[test]
    fn same_detail_reversed_twice() {
        let c = conn();
        // 第三张红票再冲蓝票第 1 行（已被 N-001 冲过一次）
        c.execute_batch(
            "INSERT INTO invoice_headers(id,e_num,seller_id,issue_date,amount,tax,total,source,inv_type,state,is_pos,risk,red_notice_no)
               VALUES (4,'R3',1,'2026-09-12',-1000,-90,-1090,1,1,1,0,1,'N-003');
             INSERT INTO invoice_details(id,header_id,line_no,goods_name,quantity,amount,tax_rate,tax,total)
               VALUES (5,4,1,'a','-1',-1000,'9%',-90,-1090);
             INSERT INTO reversal_line_matches(red_header_id,red_detail_id,blue_header_id,blue_detail_id,matched_quantity,matched_amount,matched_tax,matched_total,match_result_type,match_status,match_batch_no)
               VALUES (4,5,1,1,1,1000,90,1090,1,1,2);",
        )
        .unwrap();
        let r = query_reversal(&c, &ReversalQueryFilter::default()).unwrap();
        // 仍是 1 张发票一行、2 个明细各一行
        assert_eq!((r.summary.len(), r.details.len()), (1, 2));
        let s = &r.summary[0];
        assert_eq!((s.reversed_total.as_str(), s.remain_total.as_str()), ("5450", "5450"));
        assert_eq!(s.red_notice_nos.as_deref(), Some("N-001、N-002、N-003"));
        let d = &r.details[0];
        assert_eq!((d.reversed_amount.as_str(), d.remain_amount.as_str()), ("4000", "2000"));
        assert_eq!((d.reversed_quantity.as_deref(), d.remain_quantity.as_deref()), (Some("4"), Some("6")));
        assert_eq!(d.red_notice_nos.as_deref(), Some("N-001、N-003"));
    }

    #[test]
    fn discount_rows_are_marked_not_merged() {
        let c = conn();
        // 蓝票 B9：第 1 行 a=1000，第 2 行是 a 的折扣 -100，第 3 行又是 a 的负数（不应再被吸附），第 4 行 b=500
        // 另有两条 IC 凭证，用来确认展开凭证不会让折扣统计翻倍
        c.execute_batch(
            "INSERT INTO invoice_headers(id,e_num,seller_id,issue_date,amount,tax,total,source,inv_type,state,is_pos,risk)
               VALUES (9,'B9',1,'2026-09-15',1300,117,1417,1,1,1,1,1);
             INSERT INTO invoice_details(id,header_id,line_no,goods_name,quantity,amount,tax_rate,tax,total)
               VALUES (11,9,1,'a','1',1000,'9%',90,1090),
                      (12,9,2,'a','1',-100,'9%',-9,-109),
                      (13,9,3,'a','1',-50,'9%',-4,-54),
                      (14,9,4,'b','1',500,'9%',45,545);
             INSERT INTO ic_vouchers(header_id,legal_entity,document_no,posted_date,received_date,preparer)
               VALUES (9,'E','X1','2026-09-20','2026-09-20','x'),(9,'E','X2','2026-09-21','2026-09-21','x');",
        )
        .unwrap();
        let f = ReversalQueryFilter { invoice_number: Some("B9".into()), ..Default::default() };
        let r = query_reversal(&c, &f).unwrap();

        // 4 行明细都在，与发票查询条数一致
        assert_eq!(r.details.len(), 4);
        let flags: Vec<bool> = r.details.iter().map(|d| d.is_discount).collect();
        // 第 3 行紧跟在已经是折扣行的第 2 行之后，不再被当作折扣行
        assert_eq!(flags, vec![false, true, false, false]);

        // 折扣统计：1 笔，-100 / -9 / -109
        let d = &r.discount;
        assert_eq!((d.count, d.amount.as_str(), d.tax.as_str(), d.total.as_str()), (1, "-100", "-9", "-109"));

        // 折扣行自己没有红冲记录，累计为 0
        let disc = r.details.iter().find(|d| d.is_discount).unwrap();
        assert_eq!((disc.reversed_total.as_str(), disc.remain_total.as_str()), ("0", "-109"));

        // 展开凭证后明细行数翻倍，但折扣统计仍按明细去重
        let f = ReversalQueryFilter { expand: Some("ic".into()), ..f };
        let r = query_reversal(&c, &f).unwrap();
        assert_eq!(r.details.len(), 8);
        assert_eq!(r.discount.count, 1);
        assert_eq!(r.discount.total, "-109");
    }
}

#[cfg(test)]
mod perf {
    use super::*;
    use crate::features::reversal_query::red::{query_links, query_red};
    use std::time::Instant;

    /// 5 万张蓝票 × 5 行，2 万张红票 × 2 行，每个红字明细冲 1 个蓝字明细
    fn big() -> Connection {
        let mut c = Connection::open_in_memory().unwrap();
        unsafe {
            c.load_extension_enable().unwrap();
            let dll = concat!(env!("CARGO_MANIFEST_DIR"), "/resources/simple.dll");
            c.load_extension(dll, None::<&str>).ok();
        }
        crate::db::schema::init_db(&mut c).unwrap();
        let tx = c.transaction().unwrap();
        tx.execute("INSERT INTO sellers(id,tax_no,name) VALUES (1,'T1','甲')", []).unwrap();
        {
            let mut h = tx.prepare("INSERT INTO invoice_headers(id,e_num,seller_id,issue_date,amount,tax,total,source,inv_type,state,is_pos,risk,red_notice_no,rev_org_key) VALUES (?1,?2,1,?3,?4,?5,?6,1,1,?7,?8,1,?9,?10)").unwrap();
            let mut d = tx.prepare("INSERT INTO invoice_details(id,header_id,line_no,goods_name,quantity,amount,tax_rate,tax,total) VALUES (?1,?2,?3,'a','1',?4,'9%',?5,?6)").unwrap();
            let mut m = tx.prepare("INSERT INTO reversal_line_matches(red_header_id,red_detail_id,blue_header_id,blue_detail_id,matched_quantity,matched_amount,matched_tax,matched_total,match_result_type,match_status,match_batch_no) VALUES (?1,?2,?3,?4,1,100,9,109,1,1,1)").unwrap();
            let mut did = 0_i64;
            for b in 1..=50_000_i64 {
                let date = format!("2026-{:02}-{:02}", 1 + (b % 12), 1 + (b % 28));
                h.execute(rusqlite::params![b, format!("B{b}"), date, 500, 45, 545, 4, 1, Option::<String>::None, Option::<String>::None]).unwrap();
                for l in 1..=5 {
                    did += 1;
                    d.execute(rusqlite::params![did, b, l, 100, 9, 109]).unwrap();
                }
            }
            let mut rdid = did;
            for r in 1..=20_000_i64 {
                let rid = 50_000 + r;
                let blue = r * 2; // 红票 r 对应蓝票 2r
                let date = format!("2026-{:02}-{:02}", 1 + (r % 12), 1 + (r % 28));
                h.execute(rusqlite::params![rid, format!("R{r}"), date, -200, -18, -218, 1, 0, format!("N{r}"), format!("B{blue}")]).unwrap();
                for l in 1..=2_i64 {
                    rdid += 1;
                    d.execute(rusqlite::params![rdid, rid, l, -100, -9, -109]).unwrap();
                    // 蓝票 blue 的第 l 行（明细 id = (blue-1)*5 + l）
                    m.execute(rusqlite::params![rid, rdid, blue, (blue - 1) * 5 + l]).unwrap();
                }
            }
        }
        tx.commit().unwrap();
        c
    }

    fn plan(c: &Connection, sql: &str) {
        let mut st = c.prepare(&format!("EXPLAIN QUERY PLAN {sql}")).unwrap();
        let rows = st.query_map([], |r| r.get::<_, String>(3)).unwrap();
        for r in rows {
            println!("    PLAN: {}", r.unwrap());
        }
    }

    /// 手动跑的耗时测试（造 5 万张蓝票 / 2 万张红票）：
    ///   cargo test --lib reversal_query::repository::perf -- --ignored --nocapture
    /// debug 编译 + 内存库，只用来看量级和查询计划，不是真实库的耗时。
    #[test]
    #[ignore]
    fn timing() {
        let c = big();
        let t = Instant::now();
        let r = query_reversal(&c, &ReversalQueryFilter::default()).unwrap();
        println!("蓝字 全量   : {:>6} ms  汇总 {} 明细 {}", t.elapsed().as_millis(), r.summary.len(), r.details.len());

        let f = ReversalQueryFilter { issue_date_from: Some("2026-03-01".into()), issue_date_to: Some("2026-03-31".into()), ..Default::default() };
        let t = Instant::now();
        let r = query_reversal(&c, &f).unwrap();
        println!("蓝字 一个月 : {:>6} ms  汇总 {} 明细 {}", t.elapsed().as_millis(), r.summary.len(), r.details.len());

        let f = ReversalQueryFilter { view: Some("red".into()), ..Default::default() };
        let t = Instant::now();
        let r = query_red(&c, &f).unwrap();
        println!("红字 全量   : {:>6} ms  汇总 {} 明细 {}", t.elapsed().as_millis(), r.red_summary.len(), r.red_details.len());

        let f = ReversalQueryFilter { view: Some("red".into()), issue_date_from: Some("2026-03-01".into()), issue_date_to: Some("2026-03-31".into()), ..Default::default() };
        let t = Instant::now();
        let r = query_red(&c, &f).unwrap();
        println!("红字 一个月 : {:>6} ms  汇总 {} 明细 {}", t.elapsed().as_millis(), r.red_summary.len(), r.red_details.len());

        let t = Instant::now();
        let l = query_links(&c, &ReversalQueryFilter::default()).unwrap();
        println!("对应关系 全量: {:>6} ms  {} 行 (total {})", t.elapsed().as_millis(), l.links.len(), l.link_total);

        let f = ReversalQueryFilter { issue_date_from: Some("2026-03-01".into()), issue_date_to: Some("2026-03-31".into()), ..Default::default() };
        let t = Instant::now();
        let l = query_links(&c, &f).unwrap();
        println!("对应关系 一个月: {:>6} ms  {} 行", t.elapsed().as_millis(), l.links.len());

        let f = ReversalQueryFilter { invoice_number: Some("B1000".into()), ..Default::default() };
        let t = Instant::now();
        let l = query_links(&c, &f).unwrap();
        println!("对应关系 号码LIKE: {:>6} ms  {} 行", t.elapsed().as_millis(), l.links.len());

        println!("--- 查询计划 ---");
        println!("蓝字发票头(一个月):");
        plan(&c, "SELECT h.id FROM invoice_headers h JOIN sellers s ON s.id=h.seller_id WHERE h.is_pos=1 AND h.issue_date>='2026-03-01' AND h.issue_date<='2026-03-31' ORDER BY h.issue_date DESC, h.id DESC");
        println!("红字发票头 + LEFT JOIN 蓝票(rev_org_key = inv_unq_key):");
        plan(&c, "SELECT h.id, bh.issue_date FROM invoice_headers h LEFT JOIN invoice_headers bh ON bh.inv_unq_key=h.rev_org_key AND bh.is_pos=1 WHERE h.is_pos=0 AND h.rev_org_key IS NOT NULL");
        println!("对应关系(号码 LIKE):");
        plan(&c, "SELECT m.id FROM reversal_line_matches m JOIN invoice_headers rh ON rh.id=m.red_header_id LEFT JOIN invoice_headers bh ON bh.id=m.blue_header_id WHERE (rh.inv_unq_key LIKE '%B1000%' OR bh.inv_unq_key LIKE '%B1000%' OR rh.rev_org_key LIKE '%B1000%')");
    }
}
