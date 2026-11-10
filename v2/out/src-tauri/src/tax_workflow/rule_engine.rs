//! tax_match_rules 规则引擎（进项转出、印花税共用）
//!
//! - 一行规则内的多个条件 = AND；规则之间相互独立，逐条全部评估，命中的全部记为 hits
//! - 条件字段：
//!     seller_tax_id → sellers.tax_no
//!     tax_code      → invoice_details.tax_code（税收分类编码）
//!     tax_name      → invoice_details.goods_category（税收分类简称）；
//!                     为空时从 goods_name 的 “*简称*名称” 中提取
//! - *_values 支持多个值，分隔符：, ， ; ； | 、 换行 制表符
//! - 值中含 * 或 ? 时按通配符匹配（* 任意串，? 任意单字符），例如税收编码 "30701*"
//! - eq  = 字段值属于列表中任意一个；neq = 不属于任何一个（字段为空视为不属于）
//! - 比较前统一 trim，ASCII 字母不区分大小写

use std::collections::HashSet;

use anyhow::{bail, Context, Result};
use regex::Regex;
use rusqlite::Connection;

#[derive(Debug, Clone, Copy)]
enum Op {
    Eq,
    Neq,
}

#[derive(Debug)]
struct ValueSet {
    exact: HashSet<String>,
    globs: Vec<Regex>,
}

fn norm(s: &str) -> String {
    s.trim().to_ascii_uppercase()
}

impl ValueSet {
    fn parse(raw: &str) -> Result<Self> {
        let mut exact = HashSet::new();
        let mut globs = Vec::new();
        for tok in raw.split(|c: char| matches!(c, ',' | '，' | ';' | '；' | '|' | '、' | '\n' | '\r' | '\t')) {
            let t = norm(tok);
            if t.is_empty() {
                continue;
            }
            if t.contains('*') || t.contains('?') {
                let mut pat = String::from("^");
                for ch in t.chars() {
                    match ch {
                        '*' => pat.push_str(".*"),
                        '?' => pat.push('.'),
                        c => pat.push_str(&regex::escape(&c.to_string())),
                    }
                }
                pat.push('$');
                globs.push(Regex::new(&pat)?);
            } else {
                exact.insert(t);
            }
        }
        if exact.is_empty() && globs.is_empty() {
            bail!("规则条件值为空：{raw}");
        }
        Ok(Self { exact, globs })
    }

    fn contains(&self, v: &str) -> bool {
        let v = norm(v);
        if v.is_empty() {
            return false;
        }
        self.exact.contains(&v) || self.globs.iter().any(|g| g.is_match(&v))
    }
}

#[derive(Debug)]
struct Cond {
    op: Op,
    set: ValueSet,
}

impl Cond {
    fn build(op: Option<String>, values: Option<String>) -> Result<Option<Self>> {
        match (op.as_deref(), values) {
            (None, _) => Ok(None),
            (Some(o), Some(v)) => {
                let op = match o {
                    "eq" => Op::Eq,
                    "neq" => Op::Neq,
                    x => bail!("未知匹配方式：{x}"),
                };
                Ok(Some(Self { op, set: ValueSet::parse(&v)? }))
            }
            (Some(_), None) => bail!("规则条件缺少取值"),
        }
    }
    fn test(&self, v: Option<&str>) -> bool {
        let hit = v.map(|s| self.set.contains(s)).unwrap_or(false);
        match self.op {
            Op::Eq => hit,
            Op::Neq => !hit,
        }
    }
}

#[derive(Debug)]
pub struct CompiledRule {
    pub id: i64,
    pub category: String,
    pub stamp_category: Option<String>,
    pub needs_review: bool,
    seller: Option<Cond>,
    tax_code: Option<Cond>,
    tax_name: Option<Cond>,
}

pub struct RuleInput<'a> {
    pub seller_tax_no: &'a str,
    pub tax_code: Option<&'a str>,
    pub goods_category: Option<&'a str>,
    pub goods_name: &'a str,
}

/// 税收分类简称：优先 goods_category，否则从 “*餐饮服务*餐费” 中取 “餐饮服务”
pub fn tax_short_name<'a>(goods_category: Option<&'a str>, goods_name: &'a str) -> Option<&'a str> {
    if let Some(c) = goods_category.map(str::trim).filter(|c| !c.is_empty()) {
        return Some(c.trim_matches('*'));
    }
    let s = goods_name.trim();
    let rest = s.strip_prefix('*')?;
    let end = rest.find('*')?;
    Some(&rest[..end]).filter(|x| !x.is_empty())
}

impl CompiledRule {
    pub fn matches(&self, input: &RuleInput) -> bool {
        let name = tax_short_name(input.goods_category, input.goods_name);
        self.seller.as_ref().map_or(true, |c| c.test(Some(input.seller_tax_no)))
            && self.tax_code.as_ref().map_or(true, |c| c.test(input.tax_code))
            && self.tax_name.as_ref().map_or(true, |c| c.test(name))
    }
}

/// scope: 'input_tax_transfer' | 'stamp_tax'
pub fn load_rules(conn: &Connection, scope: &str) -> Result<Vec<CompiledRule>> {
    let mut stmt = conn.prepare(
        "SELECT id, rule_category, stamp_tax_category_code, needs_manual_review,
                seller_tax_id_match_type, seller_tax_id_values,
                tax_code_match_type, tax_code_values,
                tax_name_match_type, tax_name_values
         FROM tax_match_rules
         WHERE scope = ?1 AND enabled = 1
         ORDER BY id",
    )?;
    let raw = stmt
        .query_map([scope], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, i64>(3)?,
                r.get::<_, Option<String>>(4)?,
                r.get::<_, Option<String>>(5)?,
                r.get::<_, Option<String>>(6)?,
                r.get::<_, Option<String>>(7)?,
                r.get::<_, Option<String>>(8)?,
                r.get::<_, Option<String>>(9)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    raw.into_iter()
        .map(|(id, category, stamp, review, so, sv, co, cv, no, nv)| -> Result<CompiledRule> {
            Ok(CompiledRule {
                id,
                category,
                stamp_category: stamp,
                needs_review: review == 1,
                seller: Cond::build(so, sv).map_err(|e| e.context(format!("规则 {id}")))?,
                tax_code: Cond::build(co, cv).map_err(|e| e.context(format!("规则 {id}")))?,
                tax_name: Cond::build(no, nv).map_err(|e| e.context(format!("规则 {id}")))?,
            })
        })
        .collect()
}

pub fn hits<'r>(rules: &'r [CompiledRule], input: &RuleInput) -> Vec<&'r CompiledRule> {
    rules.iter().filter(|r| r.matches(input)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn value_set_exact_and_glob() {
        let s = ValueSet::parse("3070401000000000000， 1090* ;餐饮服务").unwrap();
        assert!(s.contains("3070401000000000000"));
        assert!(s.contains("1090511010000000000"));
        assert!(s.contains(" 餐饮服务 "));
        assert!(!s.contains("1080000"));
    }

    #[test]
    fn short_name_fallback() {
        assert_eq!(tax_short_name(None, "*餐饮服务*餐费"), Some("餐饮服务"));
        assert_eq!(tax_short_name(Some(""), "*住宿服务*住宿费"), Some("住宿服务"));
        assert_eq!(tax_short_name(Some("*烟草制品*"), "x"), Some("烟草制品"));
        assert_eq!(tax_short_name(None, "办公用品"), None);
    }

    #[test]
    fn neq_on_empty_field_is_true() {
        let c = Cond::build(Some("neq".into()), Some("A".into())).unwrap().unwrap();
        assert!(c.test(None));
        assert!(!c.test(Some("a")));
    }
}
