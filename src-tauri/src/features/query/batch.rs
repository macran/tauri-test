// 批量查询（精确匹配清单）：值在 / 不在清单中，不是模糊匹配。
//   - 清单整体作为 1 个 JSON 参数传入，用 json_each 展开：不受 SQLite 变量个数上限影响，SQL 文本固定
//   - 包含：col IN (子查询)，SQLite 把子查询建成临时索引，可以走列上的索引
//   - 不包含：NOT (COALESCE(col IN (...), 0))，col 为 NULL 时也算“不在清单中”而保留
//     （直接写 col NOT IN 遇到 NULL 会得到 NULL，整行被误丢）
use rusqlite::types::Value;
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BatchFilter {
    pub values: Vec<String>,
    /// true = 不在清单中
    pub exclude: bool,
}

/// 五个常用批量字段；各 Filter 结构体加一个 `batch: Option<BatchSet>` 即可复用
#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BatchSet {
    pub invoice_number: Option<BatchFilter>,
    pub seller_name: Option<BatchFilter>,
    pub seller_tax_no: Option<BatchFilter>,
    pub goods_name: Option<BatchFilter>,
    pub tax_code: Option<BatchFilter>,
}

/// 去首尾空白、去空值、去重（保持顺序）
fn clean(values: &[String]) -> Vec<&str> {
    let mut seen = std::collections::HashSet::new();
    values
        .iter()
        .map(|v| v.trim())
        .filter(|v| !v.is_empty() && seen.insert(*v))
        .collect()
}

/// cols 是同一个“字段”的多个候选列（如发票号码可对应 inv_unq_key / num / e_num），命中任一列即算在清单中。
/// 清单清洗后为空则不加条件。
pub fn push_batch(
    clauses: &mut Vec<String>,
    params: &mut Vec<Value>,
    cols: &[&str],
    b: &Option<BatchFilter>,
) {
    let Some(b) = b else { return };
    let vals = clean(&b.values);
    if vals.is_empty() {
        return;
    }
    let json = serde_json::to_string(&vals).expect("字符串数组序列化不会失败");
    const SET: &str = "(SELECT value FROM json_each(?))";
    let sql = if b.exclude {
        let any: Vec<String> = cols.iter().map(|c| format!("COALESCE({c} IN {SET}, 0)")).collect();
        format!("NOT ({})", any.join(" OR "))
    } else {
        let any: Vec<String> = cols.iter().map(|c| format!("{c} IN {SET}")).collect();
        format!("({})", any.join(" OR "))
    };
    clauses.push(sql);
    params.extend(std::iter::repeat(Value::Text(json)).take(cols.len()));
}

/// 发票号码：用户可能粘贴完整唯一键、纯号码或数电号码
pub const INVOICE_NO_COLS: [&str; 3] = ["h.inv_unq_key", "h.num", "h.e_num"];

impl super::repository::Where {
    pub fn batch(&mut self, cols: &[&str], b: &Option<BatchFilter>) {
        push_batch(&mut self.clauses, &mut self.params, cols, b);
    }
    /// 一次应用 BatchSet 里发票头 / 销方 / 明细的五个字段；明细列传 None 表示该页没有
    pub fn batch_set(&mut self, s: &Option<BatchSet>, goods: Option<(&str, &str)>) {
        let Some(s) = s else { return };
        self.batch(&INVOICE_NO_COLS, &s.invoice_number);
        self.batch(&["s.name"], &s.seller_name);
        self.batch(&["s.tax_no"], &s.seller_tax_no);
        if let Some((goods_col, code_col)) = goods {
            self.batch(&[goods_col], &s.goods_name);
            self.batch(&[code_col], &s.tax_code);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn run(vals: &[&str], exclude: bool) -> Vec<Option<String>> {
        let c = Connection::open_in_memory().unwrap();
        c.execute_batch("CREATE TABLE t(h_name TEXT); INSERT INTO t VALUES('a'),('b'),('c'),(NULL);")
            .unwrap();
        let (mut w, mut p) = (vec![], vec![]);
        let b = Some(BatchFilter { values: vals.iter().map(|s| s.to_string()).collect(), exclude });
        push_batch(&mut w, &mut p, &["h_name"], &b);
        let sql = format!("SELECT h_name FROM t{} ORDER BY h_name", if w.is_empty() { "".into() } else { format!(" WHERE {}", w[0]) });
        let mut st = c.prepare(&sql).unwrap();
        st.query_map(rusqlite::params_from_iter(p), |r| r.get(0)).unwrap().map(|r| r.unwrap()).collect()
    }

    #[test]
    fn include_exclude_null_and_empty() {
        assert_eq!(run(&[" a ", "b", "a", ""], false), vec![Some("a".into()), Some("b".into())]);
        // 不包含：NULL 行必须保留
        assert_eq!(run(&["a", "b"], true), vec![None, Some("c".into())]);
        // 空清单 = 不过滤
        assert_eq!(run(&["", " "], true).len(), 4);
    }
}
