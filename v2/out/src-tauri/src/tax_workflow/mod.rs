//! 进项税转出 & 印花税：匹配 → 复核 → 导出交会计 → 回填凭证确认 → (管理员解锁 → 补充批次)

pub mod common;
pub mod excel;
pub mod input_transfer;
pub mod rule_engine;
pub mod stamp_tax;
pub mod xlsx_template;

/// 发票号码展示：数电票取 e_num，旧票取 num（发票代码单独一列）
pub const INVOICE_NO_SQL: &str =
    "CASE WHEN h.e_num IS NOT NULL AND trim(h.e_num) <> '' THEN trim(h.e_num) ELSE COALESCE(trim(h.num), '') END";

/// LIKE 关键字中 % _ \ 已转义
pub const LIKE_ESCAPE: &str = r"ESCAPE '\'";
