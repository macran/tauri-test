// src-tauri/src/db/voucher_edit_operations.rs
//
// bank / AP / IC 凭证的修改与删除。
//   - 删除：凭证对应的发票只要已进入印花税结果（stamp_tax_results）或
//     进项税转出结果（welfare_transfer_results），整批拒绝，不做部分删除
//   - 修改：只改前端允许编辑的字段；AP 改了 PO/条码/供应商会影响业务指纹，
//     需同步重算 content_fingerprint，撞上已有凭证时报错而不是静默合并

use rusqlite::{params, Connection, ErrorCode};

use crate::features::ap_voucher_import::repository::{fingerprint, upsert_vendor};
use crate::features::query::{ApVoucherEdit, BankVoucherEdit, IcVoucherEdit};

fn map_err(e: rusqlite::Error) -> String {
    if let rusqlite::Error::SqliteFailure(f, _) = &e {
        if f.code == ErrorCode::ConstraintViolation {
            return "修改后与已有凭证重复（唯一约束冲突）".to_string();
        }
    }
    e.to_string()
}

/// 空白字符串视为 NULL
fn opt(v: &Option<String>) -> Option<String> {
    v.as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
}

fn required(v: &str, name: &str) -> Result<String, String> {
    let t = v.trim();
    if t.is_empty() {
        Err(format!("{name}不能为空"))
    } else {
        Ok(t.to_string())
    }
}

// ─────────────────────────────────────────────────────────────
// 删除
// ─────────────────────────────────────────────────────────────
// table 只来自下面三个固定调用，不含用户输入，可安全拼进 SQL
fn delete_vouchers(conn: &mut Connection, table: &str, ids: &[u32]) -> Result<u32, String> {
    if ids.is_empty() {
        return Ok(0);
    }
    let marks = vec!["?"; ids.len()].join(",");
    let id_params = || rusqlite::params_from_iter(ids.iter().map(|i| *i as i64));
    let tx = conn.transaction().map_err(|e| e.to_string())?;

    let blocked: i64 = tx
        .query_row(
            &format!(
                "SELECT COUNT(*) FROM {table} WHERE id IN ({marks}) AND header_id IN (
                     SELECT header_id FROM stamp_tax_results
                     UNION SELECT header_id FROM welfare_transfer_results)"
            ),
            id_params(),
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    if blocked > 0 {
        return Err(format!(
            "所选凭证中有 {blocked} 条对应的发票已参与印花税计算或进项税转出，不能删除"
        ));
    }

    let n = tx
        .execute(&format!("DELETE FROM {table} WHERE id IN ({marks})"), id_params())
        .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(n as u32)
}

pub fn delete_bank_vouchers(conn: &mut Connection, ids: &[u32]) -> Result<u32, String> {
    delete_vouchers(conn, "bank_invoice_vouchers", ids)
}
pub fn delete_ap_vouchers(conn: &mut Connection, ids: &[u32]) -> Result<u32, String> {
    delete_vouchers(conn, "ap_invoice_vouchers", ids)
}
pub fn delete_ic_vouchers(conn: &mut Connection, ids: &[u32]) -> Result<u32, String> {
    delete_vouchers(conn, "ic_vouchers", ids)
}

// ─────────────────────────────────────────────────────────────
// 修改
// ─────────────────────────────────────────────────────────────
pub fn update_bank_voucher(conn: &mut Connection, e: &BankVoucherEdit) -> Result<(), String> {
    let voucher_no = required(&e.voucher_no, "凭证号")?;
    let posted_date = required(&e.posted_date, "入账日期")?;
    let n = conn
        .execute(
            "UPDATE bank_invoice_vouchers SET voucher_no = ?1, posted_date = ?2 WHERE id = ?3",
            params![voucher_no, posted_date, e.id],
        )
        .map_err(map_err)?;
    if n == 0 {
        return Err("凭证不存在，可能已被删除".to_string());
    }
    Ok(())
}

pub fn update_ic_voucher(conn: &mut Connection, e: &IcVoucherEdit) -> Result<(), String> {
    let document_no = required(&e.document_no, "Document Number")?;
    let n = conn
        .execute(
            "UPDATE ic_vouchers SET billing = ?1, document_no = ?2, posted_date = ?3, original_po = ?4
             WHERE id = ?5",
            params![
                opt(&e.billing),
                document_no,
                opt(&e.posted_date),
                opt(&e.original_po),
                e.id
            ],
        )
        .map_err(map_err)?;
    if n == 0 {
        return Err("凭证不存在，可能已被删除".to_string());
    }
    Ok(())
}

struct ApIdentity {
    header_id: Option<i64>,
    amount: i64,
    tax: i64,
    total: i64,
    tax_rate: Option<String>,
    inv_type: u8,
    invoice_date: Option<String>,
}

pub fn update_ap_voucher(conn: &mut Connection, e: &ApVoucherEdit) -> Result<(), String> {
    let vendor_code = required(&e.vendor_code, "Vendor Code")?;
    let vendor_name = required(&e.vendor_name, "Vendor Name")?;
    let voucher_no = required(&e.voucher_no, "凭证号")?;
    let (barcode, entry_no, po_no) = (opt(&e.barcode), opt(&e.entry_no), opt(&e.po_no));

    let tx = conn.transaction().map_err(|e| e.to_string())?;

    // 指纹由「不可编辑的身份字段」+「可编辑的 PO/条码/供应商」共同决定
    let id = tx
        .query_row(
            "SELECT header_id, amount, tax, total, tax_rate, inv_type, invoice_date
             FROM ap_invoice_vouchers WHERE id = ?1",
            params![e.id],
            |r| {
                Ok(ApIdentity {
                    header_id: r.get(0)?,
                    amount: r.get(1)?,
                    tax: r.get(2)?,
                    total: r.get(3)?,
                    tax_rate: r.get(4)?,
                    inv_type: r.get::<_, i64>(5)? as u8,
                    invoice_date: r.get(6)?,
                })
            },
        )
        .map_err(|err| match err {
            rusqlite::Error::QueryReturnedNoRows => "凭证不存在，可能已被删除".to_string(),
            other => other.to_string(),
        })?;

    // 供应商编码对应的名称全局唯一：改名会同步到使用该编码的所有凭证（与导入时一致）
    let (vendor_id, _) =
        upsert_vendor(&tx, &vendor_code, &vendor_name).map_err(|e| e.to_string())?;

    let fp = fingerprint(
        id.header_id,
        barcode.as_deref(),
        vendor_id,
        id.amount,
        id.tax,
        id.total,
        id.tax_rate.as_deref(),
        id.inv_type,
        po_no.as_deref(),
        id.invoice_date.as_deref(),
    );

    // 无发票头的凭证，展示用发票号由「条码-凭证号」构成（见导入逻辑），随之重建
    tx.execute(
        "UPDATE ap_invoice_vouchers SET
            barcode = ?1, entry_no = ?2, po_no = ?3, vendor_id = ?4, voucher_no = ?5,
            entry_date = ?6, remark = ?7, tax_certified = ?8, content_fingerprint = ?9,
            invoice_no_display = CASE WHEN header_id IS NULL
                THEN COALESCE(?1, '') || '-' || ?5 ELSE invoice_no_display END
         WHERE id = ?10",
        params![
            barcode,
            entry_no,
            po_no,
            vendor_id,
            voucher_no,
            opt(&e.entry_date),
            opt(&e.remark),
            e.tax_certified,
            fp,
            e.id
        ],
    )
    .map_err(map_err)?;

    tx.commit().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    // 只建删除逻辑涉及的列，避免依赖完整 schema
    fn setup() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE bank_invoice_vouchers (id INTEGER PRIMARY KEY, header_id INTEGER);
             CREATE TABLE stamp_tax_results (id INTEGER PRIMARY KEY, header_id INTEGER);
             CREATE TABLE welfare_transfer_results (id INTEGER PRIMARY KEY, header_id INTEGER);
             INSERT INTO bank_invoice_vouchers VALUES (1, 10), (2, 20), (3, 30);
             INSERT INTO stamp_tax_results VALUES (1, 20);
             INSERT INTO welfare_transfer_results VALUES (1, 30);",
        )
        .unwrap();
        conn
    }

    #[test]
    fn delete_rejects_whole_batch_when_any_invoice_already_calculated() {
        let mut conn = setup();
        assert!(delete_bank_vouchers(&mut conn, &[1, 2]).is_err());
        assert!(delete_bank_vouchers(&mut conn, &[1, 3]).is_err());
        let left: i64 = conn
            .query_row("SELECT COUNT(*) FROM bank_invoice_vouchers", [], |r| r.get(0))
            .unwrap();
        assert_eq!(left, 3);
    }

    #[test]
    fn delete_removes_uncalculated_vouchers() {
        let mut conn = setup();
        assert_eq!(delete_bank_vouchers(&mut conn, &[1]).unwrap(), 1);
    }
}
