use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use sha2::{Digest, Sha256};

use super::rules_models::{
    NormalizedRule,
    StampTaxCategoryOption,
    TaxMatchRuleRecord,
};
pub(crate) fn resolve_stamp_tax_category(
    tx: &Transaction<'_>,
    rule: &mut NormalizedRule,
) -> Result<(), String> {
    if rule.scope != "stamp_tax" {
        rule.stamp_tax_category_id = None;
        return Ok(());
    }

    let category_id: i64 = tx
        .query_row(
            "SELECT id
             FROM stamp_tax_categories
             WHERE category_name = ?1 AND enabled = 1",
            params![rule.rule_category],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| format!("查询印花税类别失败：{error}"))?
        .ok_or_else(|| {
            format!(
                "印花税类别“{}”不存在，或该类别已禁用",
                rule.rule_category
            )
        })?;

    rule.stamp_tax_category_id = Some(category_id);
    Ok(())
}

// #[derive(serde::Serialize)]
// struct Signature<'a> {
//     scope: &'a str,
//     rule_category: &'a str,
//     stamp_tax_category_id: &'a Option<i64>,
//     seller_tax_id_match_type: &'a Option<String>,
//     seller_tax_id_values: &'a Option<String>,
//     tax_code_match_type: &'a Option<String>,
//     tax_code_values: &'a Option<String>,
//     tax_name_match_type: &'a Option<String>,
//     tax_name_values: &'a Option<String>,
// }
 #[derive(serde::Serialize)]
    struct SignaturePayload<'a> {
        scope: &'a str,
        rule_category: &'a str,
        stamp_tax_category_id: Option<i64>,

        seller_tax_id_match_type: Option<&'a str>,
        seller_tax_id_values: Option<&'a str>,

        tax_code_match_type: Option<&'a str>,
        tax_code_values: Option<&'a str>,

        tax_name_match_type: Option<&'a str>,
        tax_name_values: Option<&'a str>,
    }

fn signature(rule: &NormalizedRule) -> Result<Vec<u8>, String> {
   
    let payload = SignaturePayload {
        scope: &rule.scope,
        rule_category: &rule.rule_category,
        stamp_tax_category_id: rule.stamp_tax_category_id,

        seller_tax_id_match_type: rule
            .seller_tax_id_match_type
            .as_deref(),
        seller_tax_id_values: rule
            .seller_tax_id_values
            .as_deref(),

        tax_code_match_type: rule
            .tax_code_match_type
            .as_deref(),
        tax_code_values: rule
            .tax_code_values
            .as_deref(),

        tax_name_match_type: rule
            .tax_name_match_type
            .as_deref(),
        tax_name_values: rule
            .tax_name_values
            .as_deref(),
    };

    let canonical_bytes = serde_json::to_vec(&payload)
        .map_err(|error| format!("生成规则签名内容失败：{error}"))?;

    let digest = Sha256::digest(&canonical_bytes);

    Ok(digest.to_vec())
}

pub(crate) fn insert_rule(
    tx: &Transaction<'_>,
    rule: &mut NormalizedRule,
    source: &str,
) -> Result<Option<i64>, String> {
    resolve_stamp_tax_category(tx, rule)?;
    let rule_signature = signature(rule)?;

    let duplicated: bool = tx
        .query_row(
            "SELECT EXISTS(
                SELECT 1 FROM tax_match_rules
                WHERE enabled = 1 AND rule_signature = ?1
            )",
            params![rule_signature],
            |row| row.get(0),
        )
        .map_err(|error| format!("检查启用规则重复失败：{error}"))?;

    if duplicated {
        return Ok(None);
    }

    let now = Utc::now().to_rfc3339();

   
    tx.execute(
        "INSERT INTO tax_match_rules (
            scope, rule_category, stamp_tax_category_id,
            seller_tax_id_match_type, seller_tax_id_values,
            tax_code_match_type, tax_code_values,
            tax_name_match_type, tax_name_values,
            needs_manual_review, note,
            enabled, disabled_at,
            source, created_at, rule_signature
        ) VALUES (
            ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11,
            1, NULL, ?13, ?12, ?14
        )",
        params![
            rule.scope,
            rule.rule_category,
            rule.stamp_tax_category_id,
            rule.seller_tax_id_match_type,
            rule.seller_tax_id_values,
            rule.tax_code_match_type,
            rule.tax_code_values,
            rule.tax_name_match_type,
            rule.tax_name_values,
            rule.needs_manual_review as i32,
            rule.note,
            now,
            source,
            rule_signature,
        ],
    )
    .map_err(|error| format!("保存规则失败：{error}"))?;

    Ok(Some(tx.last_insert_rowid()))
}

fn select_columns() -> &'static str {
    "id, scope, rule_category, stamp_tax_category_id,
     seller_tax_id_match_type, seller_tax_id_values,
     tax_code_match_type, tax_code_values,
     tax_name_match_type, tax_name_values,
     needs_manual_review, note, enabled,
     disabled_at, source, created_at"
}

fn map_rule(row: &rusqlite::Row<'_>) -> rusqlite::Result<TaxMatchRuleRecord> {
    Ok(TaxMatchRuleRecord {
         id: row.get::<_, u32>(0)?,
        scope: row.get(1)?,
        rule_category: row.get(2)?,
        stamp_tax_category_id: row.get(3)?,
        seller_tax_id_match_type: row.get(4)?,
        seller_tax_id_values: row.get(5)?,
        tax_code_match_type: row.get(6)?,
        tax_code_values: row.get(7)?,
        tax_name_match_type: row.get(8)?,
        tax_name_values: row.get(9)?,
        needs_manual_review: row.get::<_, i32>(10)? != 0,
        note: row.get(11)?,
        enabled: row.get::<_, i32>(12)? != 0,
        disabled_at: row.get(13)?,
        source: row.get(14)?,
        created_at: row.get(15)?,
    })
}

pub(crate) fn get_rule(
    conn: &Connection,
    id: i64,
) -> Result<Option<TaxMatchRuleRecord>, String> {
    let sql = format!("SELECT {} FROM tax_match_rules WHERE id = ?1", select_columns());
    conn.query_row(&sql, params![id], map_rule)
        .optional()
        .map_err(|error| format!("读取规则失败：{error}"))
}

pub(crate) fn list_rules(
    conn: &Connection,
    include_disabled: bool,
) -> Result<Vec<TaxMatchRuleRecord>, String> {
    let sql = if include_disabled {
        format!(
            "SELECT {} FROM tax_match_rules ORDER BY enabled DESC, id DESC",
            select_columns()
        )
    } else {
        format!(
            "SELECT {} FROM tax_match_rules WHERE enabled = 1 ORDER BY id DESC",
            select_columns()
        )
    };

    let mut statement = conn
        .prepare(&sql)
        .map_err(|error| format!("准备规则查询失败：{error}"))?;
    let rows = statement
        .query_map([], map_rule)
        .map_err(|error| format!("查询规则失败：{error}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("读取规则结果失败：{error}"))
}

pub(crate) fn disable_rule(conn: &Connection, id: i64) -> Result<(), String> {
    let disabled_at = Utc::now().to_rfc3339();
    let changed = conn
        .execute(
            "UPDATE tax_match_rules
             SET enabled = 0, disabled_at = ?1
             WHERE id = ?2 AND enabled = 1",
            params![disabled_at, id],
        )
        .map_err(|error| format!("禁用规则失败：{error}"))?;

    if changed == 0 {
        return Err("规则不存在，或已经被禁用；禁用后的规则不能重新启用".to_string());
    }
    Ok(())
}

pub(crate) fn list_stamp_tax_categories(
    conn: &Connection,
) -> Result<Vec<StampTaxCategoryOption>, String> {
    let mut statement = conn
        .prepare(
            "
            SELECT id, category_name, rate_bp
            FROM stamp_tax_categories
            WHERE enabled = 1
            ORDER BY id
            ",
        )
        .map_err(|error| {
            format!("准备印花税类别查询失败：{error}")
        })?;

    let rows = statement
        .query_map([], |row| {
            Ok(StampTaxCategoryOption {
                id: row.get::<_, u32>(0)?,
                category_name: row.get(1)?,
                rate_bp: row.get::<_, u32>(2)?,
            })
        })
        .map_err(|error| {
            format!("查询印花税类别失败：{error}")
        })?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            format!("读取印花税类别失败：{error}")
        })
}