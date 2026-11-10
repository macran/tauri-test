// src-tauri/src/db/schema_tax_workflow.rs
//
// 进项税转出 / 印花税 —— 「纳税所属期批次 → 来源期间匹配 → 会计凭证批次 → 确认锁定」流程的建表语句。
//
// 取代旧版：
//   - tax_period_closings / tax_period_confirmations（单期间、无凭证信息，表达不了跨期与补税）
//   - welfare_transfer_* / stamp_tax_* 中的 tax_period 列（改为 run_id 外键）
//
// 核心概念
// ┌─────────────────┐ 1   n ┌──────────────────┐ 1   n ┌─────────────────────────┐
// │  tax_runs       │──────▶│ tax_run_batches   │──────▶│ *_confirmed_lines        │
// │ 纳税所属期结果集  │       │ 会计凭证批次        │       │ 已确认明细快照(不可改删)  │
// └──────────────┘       └──────────────────┘       └─────────────────────────┘
//   tax_runs 的主键批次对应纳税所属期；filing_period_from/to 在首次匹配前确定
//   tax_run_sources 保存该纳税所属期下匹配过的认证期间/凭证期间，可追加且可跨期
//   status: open(从未确认) → confirmed(锁定) → reopened(管理员解锁，只能新增) → confirmed …
//
// 批次：batch_no = 1 为首次确认凭证；解锁后每次补税新增一个批次(2、3…)。
//       同一流程(run)同时最多一个「待确认」批次；同一来源期间可在后续纳税期新建流程。
//
// 「结果 / 命中 / 人工」三表模式保持不变，只是 tax_period → run_id：
//   *_results    系统匹配事实；重新匹配时删除重建（已确认、已人工判定的行除外）
//   *_rule_hits  命中规则外键
//   *_overrides  人工结论；重新匹配绝不触碰；有 included/税额/类别 判定的行不再参与匹配
//
// *_confirmed_lines 是确认时刻的快照（会计已据此做账），由触发器保证不可 UPDATE / DELETE。

use rusqlite::{Connection, Result as SqlResult};

// ─────────────────────────────────────────────────────────────
// 管理员 / 通用设置
// ─────────────────────────────────────────────────────────────
const CREATE_APP_ADMINS_AND_SETTINGS: &str = "
-- 管理员名单（Windows 用户名，大写，同 AppRuntimeState.username）。
-- 第一个管理员需手工插入：INSERT INTO app_admins VALUES ('ZHANGSAN', 'init', datetime('now','localtime'));
CREATE TABLE IF NOT EXISTS app_admins (
    username  TEXT NOT NULL PRIMARY KEY COLLATE NOCASE,
    added_by  TEXT NOT NULL,
    added_at  TEXT NOT NULL
);

-- 通用 KV 设置（JSON 值）。目前用于：stamp_tax.collection_template（印花税信息采集表模版映射）
CREATE TABLE IF NOT EXISTS app_settings (
    key         TEXT NOT NULL PRIMARY KEY,
    value       TEXT NOT NULL,
    updated_by  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);
";

// ─────────────────────────────────────────────────────────────
// 纳税所属期批次 / 会计凭证批次 / 来源期间 / 事件日志
// ─────────────────────────────────────────────────────────────
const CREATE_TAX_RUNS: &str = "
CREATE TABLE IF NOT EXISTS tax_runs (
    id                  INTEGER PRIMARY KEY,
    module              TEXT NOT NULL CHECK (module IN ('input_tax_transfer', 'stamp_tax')),

    -- 兼容旧版本的首个来源范围镜像；新逻辑以 tax_run_sources 为准，不作为批次键
    src_period_from     TEXT NOT NULL CHECK (length(src_period_from) = 6),
    src_period_to       TEXT NOT NULL CHECK (length(src_period_to) = 6),

    -- 纳税所属期 YYYYMM，创建批次前选定
    filing_period_from  TEXT CHECK (filing_period_from IS NULL OR length(filing_period_from) = 6),
    filing_period_to    TEXT CHECK (filing_period_to   IS NULL OR length(filing_period_to)   = 6),

    status              TEXT NOT NULL DEFAULT 'open'
                        CHECK (status IN ('open', 'confirmed', 'reopened')),

    created_by          TEXT NOT NULL,
    created_at          TEXT NOT NULL,
    last_matched_at     TEXT,
    last_modified_at    TEXT,

    CHECK (src_period_to >= src_period_from),
    CHECK (
        (filing_period_from IS NULL AND filing_period_to IS NULL)
        OR (filing_period_from IS NOT NULL AND filing_period_to IS NOT NULL
            AND filing_period_to >= filing_period_from)
    ),
    CHECK (status = 'open' OR filing_period_from IS NOT NULL)
);
-- 纳税所属期是批次唯一键；匹配来源期间记录在 tax_run_sources 中。
CREATE INDEX IF NOT EXISTS idx_tax_runs_source
    ON tax_runs(module, src_period_from, src_period_to);
DROP INDEX IF EXISTS ux_tax_runs_filing;
CREATE UNIQUE INDEX ux_tax_runs_filing
    ON tax_runs(module, filing_period_from, filing_period_to) WHERE filing_period_from IS NOT NULL;

-- 一个纳税所属期结果集可以追加多个认证期间/凭证期间；来源范围不是批次主键。
CREATE TABLE IF NOT EXISTS tax_run_sources (
    id               INTEGER PRIMARY KEY,
    run_id           INTEGER NOT NULL,
    src_period_from  TEXT NOT NULL CHECK (length(src_period_from) = 6),
    src_period_to    TEXT NOT NULL CHECK (length(src_period_to) = 6),
    added_by         TEXT NOT NULL,
    added_at         TEXT NOT NULL,
    FOREIGN KEY (run_id) REFERENCES tax_runs(id) ON DELETE CASCADE,
    CHECK (src_period_to >= src_period_from),
    UNIQUE (run_id, src_period_from, src_period_to)
);
CREATE INDEX IF NOT EXISTS idx_tax_run_sources_range
    ON tax_run_sources(src_period_from, src_period_to);
CREATE INDEX IF NOT EXISTS idx_tax_runs_module_status ON tax_runs(module, status);

CREATE TABLE IF NOT EXISTS tax_run_batches (
    id                  INTEGER PRIMARY KEY,
    run_id              INTEGER NOT NULL,
    batch_no            INTEGER NOT NULL CHECK (batch_no >= 1),   -- 1 = 首次凭证，>1 = 补税凭证
    created_at          TEXT    NOT NULL,

    -- 最近一次导出（交会计做账）的快照；确认时比对指纹，防止导出后又改了结果
    exported_at         TEXT,
    exported_by         TEXT,
    export_fingerprint  BLOB CHECK (export_fingerprint IS NULL OR length(export_fingerprint) = 32),
    export_line_count   INTEGER,
    export_tax_total    INTEGER,

    -- 会计做账信息（会计做完账后由用户回填）。本批 0 行时允许为空。
    acct_voucher_no     TEXT,
    acct_posted_date    TEXT,
    acct_preparer       TEXT,

    confirmed_by        TEXT,
    confirmed_at        TEXT,

    FOREIGN KEY (run_id) REFERENCES tax_runs(id) ON DELETE CASCADE,
    UNIQUE (run_id, batch_no),
    CHECK ((exported_at IS NULL) = (export_fingerprint IS NULL)),
    CHECK ((confirmed_at IS NULL) = (confirmed_by IS NULL)),
    CHECK (confirmed_at IS NULL OR exported_at IS NOT NULL)
);
-- 每个纳税所属期结果集同时最多一个待确认凭证批次
CREATE UNIQUE INDEX IF NOT EXISTS ux_trb_one_pending
    ON tax_run_batches(run_id) WHERE confirmed_at IS NULL;

CREATE TABLE IF NOT EXISTS tax_run_events (
    id        INTEGER PRIMARY KEY,
    run_id    INTEGER NOT NULL,
    event     TEXT    NOT NULL CHECK (event IN
                ('created', 'matched', 'exported', 'confirmed', 'unlocked', 'relocked')),
    detail    TEXT    NOT NULL DEFAULT '',
    actor     TEXT    NOT NULL,
    at        TEXT    NOT NULL,
    FOREIGN KEY (run_id) REFERENCES tax_runs(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_tax_run_events_run ON tax_run_events(run_id);
";

// ─────────────────────────────────────────────────────────────
// 四种凭证统一视图：AP / IC / 银行 / 差旅(certified_list 中 is_travel_expense = 1)
// posted_period = 入账日期规范化后的 YYYYMM（兼容 2026-10-15 / 2026/10/15 / 20261015）
// ─────────────────────────────────────────────────────────────
const CREATE_VOUCHER_VIEW: &str = "
DROP VIEW IF EXISTS v_invoice_vouchers;
CREATE VIEW v_invoice_vouchers AS
SELECT header_id, 'AP' AS source, voucher_no, entry_date AS posted_date,
       substr(replace(replace(entry_date, '-', ''), '/', ''), 1, 6) AS posted_period,
       po_no, cost_center
FROM ap_invoice_vouchers
WHERE header_id IS NOT NULL
UNION ALL
SELECT header_id, 'IC', document_no, posted_date,
       substr(replace(replace(posted_date, '-', ''), '/', ''), 1, 6),
       original_po, cost_center
FROM ic_vouchers
UNION ALL
SELECT header_id, 'BANK', voucher_no, posted_date,
       substr(replace(replace(posted_date, '-', ''), '/', ''), 1, 6),
       NULL, NULL
FROM bank_invoice_vouchers
UNION ALL
SELECT header_id, 'TE', voucher_no, posted_date,
       substr(replace(replace(posted_date, '-', ''), '/', ''), 1, 6),
       NULL, NULL
FROM certified_list
WHERE is_travel_expense = 1 AND voucher_no IS NOT NULL AND trim(voucher_no) <> '';
";

// ─────────────────────────────────────────────────────────────
// 进项税转出
// ─────────────────────────────────────────────────────────────
const CREATE_WELFARE_TRANSFER_TABLES: &str = "
CREATE TABLE IF NOT EXISTS welfare_transfer_results (
    id          INTEGER PRIMARY KEY,
    run_id      INTEGER NOT NULL,
    detail_id   INTEGER NOT NULL,
    header_id   INTEGER NOT NULL,
    matched_at  TEXT    NOT NULL,
    FOREIGN KEY (run_id)    REFERENCES tax_runs(id),
    FOREIGN KEY (detail_id) REFERENCES invoice_details(id),
    FOREIGN KEY (header_id) REFERENCES invoice_headers(id),
    UNIQUE (detail_id)
);
CREATE INDEX IF NOT EXISTS idx_wtr_run    ON welfare_transfer_results(run_id);
CREATE INDEX IF NOT EXISTS idx_wtr_header ON welfare_transfer_results(header_id);

CREATE TABLE IF NOT EXISTS welfare_transfer_rule_hits (
    id        INTEGER PRIMARY KEY,
    result_id INTEGER NOT NULL,
    rule_id   INTEGER NOT NULL,
    FOREIGN KEY (result_id) REFERENCES welfare_transfer_results(id) ON DELETE CASCADE,
    FOREIGN KEY (rule_id)   REFERENCES tax_match_rules(id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    UNIQUE (result_id, rule_id)
);
CREATE INDEX IF NOT EXISTS idx_wtrh_result ON welfare_transfer_rule_hits(result_id);
CREATE INDEX IF NOT EXISTS idx_wtrh_rule   ON welfare_transfer_rule_hits(rule_id);

-- included NULL = 未判定；transfer_tax NULL = 沿用明细税额
CREATE TABLE IF NOT EXISTS welfare_transfer_overrides (
    id            INTEGER PRIMARY KEY,
    run_id        INTEGER NOT NULL,
    detail_id     INTEGER NOT NULL,
    included      INTEGER CHECK (included IN (0, 1)),
    transfer_tax  INTEGER CHECK (transfer_tax IS NULL OR transfer_tax >= 0),
    remark        TEXT,
    updated_by    TEXT NOT NULL,
    updated_at    TEXT NOT NULL,
    FOREIGN KEY (run_id)    REFERENCES tax_runs(id),
    FOREIGN KEY (detail_id) REFERENCES invoice_details(id),
    UNIQUE (run_id, detail_id),
    CHECK (
        included IS NOT NULL
        OR transfer_tax IS NOT NULL
        OR (remark IS NOT NULL AND length(trim(remark)) > 0)
    )
);
CREATE INDEX IF NOT EXISTS idx_wto_run ON welfare_transfer_overrides(run_id);

-- 已确认转出明细（只存转出的行）。会计已据此做账，永不修改。
CREATE TABLE IF NOT EXISTS welfare_transfer_confirmed_lines (
    id            INTEGER PRIMARY KEY,
    run_id        INTEGER NOT NULL,
    batch_id      INTEGER NOT NULL,
    detail_id     INTEGER NOT NULL,
    header_id     INTEGER NOT NULL,
    cert_period   TEXT    NOT NULL,              -- 认证期间快照
    detail_tax    INTEGER NOT NULL,              -- 明细原税额（分）
    transfer_tax  INTEGER NOT NULL,              -- 转出税额（分）
    decision      TEXT    NOT NULL CHECK (decision IN ('auto', 'reviewed', 'manual')),
    remark        TEXT,
    confirmed_at  TEXT    NOT NULL,
    FOREIGN KEY (run_id)    REFERENCES tax_runs(id),
    FOREIGN KEY (batch_id)  REFERENCES tax_run_batches(id),
    FOREIGN KEY (detail_id) REFERENCES invoice_details(id),
    FOREIGN KEY (header_id) REFERENCES invoice_headers(id),
    UNIQUE (detail_id)
);
CREATE INDEX IF NOT EXISTS idx_wtcl_run   ON welfare_transfer_confirmed_lines(run_id);
CREATE INDEX IF NOT EXISTS idx_wtcl_batch ON welfare_transfer_confirmed_lines(batch_id);

CREATE TRIGGER IF NOT EXISTS trg_wtcl_no_update
BEFORE UPDATE ON welfare_transfer_confirmed_lines
BEGIN SELECT RAISE(ABORT, '已确认的进项转出明细不可修改'); END;

CREATE TRIGGER IF NOT EXISTS trg_wtcl_no_delete
BEFORE DELETE ON welfare_transfer_confirmed_lines
BEGIN SELECT RAISE(ABORT, '已确认的进项转出明细不可删除'); END;

-- 已确认明细的匹配事实 / 人工结论同样冻结（保证查询时命中规则、备注不丢）
CREATE TRIGGER IF NOT EXISTS trg_wtr_protect_confirmed
BEFORE DELETE ON welfare_transfer_results
WHEN EXISTS (SELECT 1 FROM welfare_transfer_confirmed_lines c WHERE c.detail_id = OLD.detail_id)
BEGIN SELECT RAISE(ABORT, '该明细已确认转出，匹配结果不可删除'); END;

CREATE TRIGGER IF NOT EXISTS trg_wto_protect_confirmed_upd
BEFORE UPDATE ON welfare_transfer_overrides
WHEN EXISTS (SELECT 1 FROM welfare_transfer_confirmed_lines c WHERE c.detail_id = OLD.detail_id)
BEGIN SELECT RAISE(ABORT, '该明细已确认转出，不可修改'); END;

CREATE TRIGGER IF NOT EXISTS trg_wto_protect_confirmed_del
BEFORE DELETE ON welfare_transfer_overrides
WHEN EXISTS (SELECT 1 FROM welfare_transfer_confirmed_lines c WHERE c.detail_id = OLD.detail_id)
BEGIN SELECT RAISE(ABORT, '该明细已确认转出，不可修改'); END;
";

// ─────────────────────────────────────────────────────────────
// 印花税（与进项转出完全对称）
// ─────────────────────────────────────────────────────────────
const CREATE_STAMP_TAX_TABLES: &str = "
CREATE TABLE IF NOT EXISTS stamp_tax_results (
    id          INTEGER PRIMARY KEY,
    run_id      INTEGER NOT NULL,
    detail_id   INTEGER NOT NULL,
    header_id   INTEGER NOT NULL,
    matched_at  TEXT    NOT NULL,
    FOREIGN KEY (run_id)    REFERENCES tax_runs(id),
    FOREIGN KEY (detail_id) REFERENCES invoice_details(id),
    FOREIGN KEY (header_id) REFERENCES invoice_headers(id),
    UNIQUE (detail_id)
);
CREATE INDEX IF NOT EXISTS idx_str_run    ON stamp_tax_results(run_id);
CREATE INDEX IF NOT EXISTS idx_str_header ON stamp_tax_results(header_id);

CREATE TABLE IF NOT EXISTS stamp_tax_rule_hits (
    id        INTEGER PRIMARY KEY,
    result_id INTEGER NOT NULL,
    rule_id   INTEGER NOT NULL,
    FOREIGN KEY (result_id) REFERENCES stamp_tax_results(id) ON DELETE CASCADE,
    FOREIGN KEY (rule_id)   REFERENCES tax_match_rules(id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    UNIQUE (result_id, rule_id)
);
CREATE INDEX IF NOT EXISTS idx_strh_result ON stamp_tax_rule_hits(result_id);
CREATE INDEX IF NOT EXISTS idx_strh_rule   ON stamp_tax_rule_hits(rule_id);

CREATE TABLE IF NOT EXISTS stamp_tax_overrides (
    id                       INTEGER PRIMARY KEY,
    run_id                   INTEGER NOT NULL,
    detail_id                INTEGER NOT NULL,
    included                 INTEGER CHECK (included IN (0, 1)),
    stamp_tax_category_code  TEXT,
    remark                   TEXT,
    updated_by               TEXT NOT NULL,
    updated_at               TEXT NOT NULL,
    FOREIGN KEY (run_id)    REFERENCES tax_runs(id),
    FOREIGN KEY (detail_id) REFERENCES invoice_details(id),
    FOREIGN KEY (stamp_tax_category_code) REFERENCES stamp_tax_categories(code),
    UNIQUE (run_id, detail_id),
    CHECK (
        included IS NOT NULL
        OR stamp_tax_category_code IS NOT NULL
        OR (remark IS NOT NULL AND length(trim(remark)) > 0)
    )
);
CREATE INDEX IF NOT EXISTS idx_sto_run ON stamp_tax_overrides(run_id);

CREATE TABLE IF NOT EXISTS stamp_tax_confirmed_lines (
    id             INTEGER PRIMARY KEY,
    run_id         INTEGER NOT NULL,
    batch_id       INTEGER NOT NULL,
    detail_id      INTEGER NOT NULL,
    header_id      INTEGER NOT NULL,
    category_code  TEXT    NOT NULL,             -- 子目（或无子目的税目）
    base_amount    INTEGER NOT NULL,             -- 计税金额（分）= 明细不含税金额
    rate_ppm       INTEGER NOT NULL,             -- 税率（百万分之）
    stamp_tax      INTEGER NOT NULL,             -- 行级印花税（分，仅参考；采集表按汇总计算）
    decision       TEXT    NOT NULL CHECK (decision IN ('auto', 'reviewed', 'manual')),
    remark         TEXT,
    confirmed_at   TEXT    NOT NULL,
    FOREIGN KEY (run_id)        REFERENCES tax_runs(id),
    FOREIGN KEY (batch_id)      REFERENCES tax_run_batches(id),
    FOREIGN KEY (detail_id)     REFERENCES invoice_details(id),
    FOREIGN KEY (header_id)     REFERENCES invoice_headers(id),
    FOREIGN KEY (category_code) REFERENCES stamp_tax_categories(code),
    UNIQUE (detail_id)
);
CREATE INDEX IF NOT EXISTS idx_stcl_run   ON stamp_tax_confirmed_lines(run_id);
CREATE INDEX IF NOT EXISTS idx_stcl_batch ON stamp_tax_confirmed_lines(batch_id);

CREATE TRIGGER IF NOT EXISTS trg_stcl_no_update
BEFORE UPDATE ON stamp_tax_confirmed_lines
BEGIN SELECT RAISE(ABORT, '已确认的印花税明细不可修改'); END;

CREATE TRIGGER IF NOT EXISTS trg_stcl_no_delete
BEFORE DELETE ON stamp_tax_confirmed_lines
BEGIN SELECT RAISE(ABORT, '已确认的印花税明细不可删除'); END;

CREATE TRIGGER IF NOT EXISTS trg_str_protect_confirmed
BEFORE DELETE ON stamp_tax_results
WHEN EXISTS (SELECT 1 FROM stamp_tax_confirmed_lines c WHERE c.detail_id = OLD.detail_id)
BEGIN SELECT RAISE(ABORT, '该明细已确认计入印花税，匹配结果不可删除'); END;

CREATE TRIGGER IF NOT EXISTS trg_sto_protect_confirmed_upd
BEFORE UPDATE ON stamp_tax_overrides
WHEN EXISTS (SELECT 1 FROM stamp_tax_confirmed_lines c WHERE c.detail_id = OLD.detail_id)
BEGIN SELECT RAISE(ABORT, '该明细已确认计入印花税，不可修改'); END;

CREATE TRIGGER IF NOT EXISTS trg_sto_protect_confirmed_del
BEFORE DELETE ON stamp_tax_overrides
WHEN EXISTS (SELECT 1 FROM stamp_tax_confirmed_lines c WHERE c.detail_id = OLD.detail_id)
BEGIN SELECT RAISE(ABORT, '该明细已确认计入印花税，不可修改'); END;
";

// ─────────────────────────────────────────────────────────────
// 迁移辅助
// ─────────────────────────────────────────────────────────────
fn table_exists(conn: &Connection, table: &str) -> SqlResult<bool> {
    conn.prepare("SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1")?
        .exists([table])
}

fn has_column(conn: &Connection, table: &str, column: &str) -> SqlResult<bool> {
    conn.prepare("SELECT 1 FROM pragma_table_info(?1) WHERE name = ?2")?
        .exists(rusqlite::params![table, column])
}

/// Previous patch versions made the source range unique on tax_runs. Rebuild that table
/// safely so a confirmed source period can have a new filing-period workflow later.
fn migrate_tax_runs_source_reuse(conn: &Connection) -> SqlResult<()> {
    if !table_exists(conn, "tax_runs")? {
        return Ok(());
    }
    let ddl: String = conn.query_row(
        "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'tax_runs'",
        [],
        |r| r.get(0),
    )?;
    let compact: String = ddl.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_ascii_lowercase();
    if !compact.contains("unique(module,src_period_from,src_period_to)") {
        return Ok(());
    }

    conn.execute_batch("PRAGMA foreign_keys = OFF;")?;
    let migration = conn.execute_batch(
        "BEGIN IMMEDIATE;
         CREATE TABLE tax_runs_rebuild (
             id                  INTEGER PRIMARY KEY,
             module              TEXT NOT NULL CHECK (module IN ('input_tax_transfer', 'stamp_tax')),
             src_period_from     TEXT NOT NULL CHECK (length(src_period_from) = 6),
             src_period_to       TEXT NOT NULL CHECK (length(src_period_to) = 6),
             filing_period_from  TEXT CHECK (filing_period_from IS NULL OR length(filing_period_from) = 6),
             filing_period_to    TEXT CHECK (filing_period_to IS NULL OR length(filing_period_to) = 6),
             status              TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'confirmed', 'reopened')),
             created_by          TEXT NOT NULL,
             created_at          TEXT NOT NULL,
             last_matched_at     TEXT,
             last_modified_at    TEXT,
             CHECK (src_period_to >= src_period_from),
             CHECK ((filing_period_from IS NULL AND filing_period_to IS NULL)
                 OR (filing_period_from IS NOT NULL AND filing_period_to IS NOT NULL
                     AND filing_period_to >= filing_period_from)),
             CHECK (status = 'open' OR filing_period_from IS NOT NULL)
         );
         INSERT INTO tax_runs_rebuild (
             id, module, src_period_from, src_period_to, filing_period_from, filing_period_to,
             status, created_by, created_at, last_matched_at, last_modified_at
         )
         SELECT id, module, src_period_from, src_period_to, filing_period_from, filing_period_to,
                status, created_by, created_at, last_matched_at, last_modified_at
         FROM tax_runs;
         DROP TABLE tax_runs;
         ALTER TABLE tax_runs_rebuild RENAME TO tax_runs;
         CREATE INDEX IF NOT EXISTS idx_tax_runs_source ON tax_runs(module, src_period_from, src_period_to);
         CREATE UNIQUE INDEX IF NOT EXISTS ux_tax_runs_filing
             ON tax_runs(module, filing_period_from) WHERE filing_period_from IS NOT NULL;
         CREATE INDEX IF NOT EXISTS idx_tax_runs_module_status ON tax_runs(module, status);
         COMMIT;"
    );
    if let Err(e) = migration {
        let _ = conn.execute_batch("ROLLBACK;");
        let _ = conn.execute_batch("PRAGMA foreign_keys = ON;");
        return Err(e);
    }
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;
    Ok(())
}

/// 旧结构表（无 run_id）：空表直接 DROP；有数据则报错，提示人工处理，绝不静默丢数据。
fn drop_if_stale_and_empty(conn: &Connection, table: &str, stale: bool) -> SqlResult<()> {
    if !stale || !table_exists(conn, table)? {
        return Ok(());
    }
    let n: i64 = conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))?;
    if n > 0 {
        return Err(rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_SCHEMA),
            Some(format!(
                "表 {table} 是旧结构（按 tax_period 存储）且有 {n} 行数据，无法自动迁移到纳税所属期结果集(run_id)模型，请备份后手工处理"
            )),
        ));
    }
    conn.execute_batch(&format!("DROP TABLE {table};"))
}

fn migrate_legacy(conn: &Connection) -> SqlResult<()> {
    // 子表先删（外键）
    for (child, parent) in [
        ("welfare_transfer_rule_hits", "welfare_transfer_results"),
        ("stamp_tax_rule_hits", "stamp_tax_results"),
    ] {
        let parent_stale = table_exists(conn, parent)? && !has_column(conn, parent, "run_id")?;
        drop_if_stale_and_empty(conn, child, parent_stale)?;
    }
    for t in [
        "welfare_transfer_results",
        "welfare_transfer_overrides",
        "stamp_tax_results",
        "stamp_tax_overrides",
    ] {
        let stale = table_exists(conn, t)? && !has_column(conn, t, "run_id")?;
        drop_if_stale_and_empty(conn, t, stale)?;
    }
    // 旧的月结 / 确认表已被 tax_runs + tax_run_batches 取代
    for t in ["tax_period_closings", "tax_period_confirmations"] {
        drop_if_stale_and_empty(conn, t, true)?;
    }
    // 成本中心：印花税清单需要。导入 AP / IC 凭证时请填充该列（不参与 content_fingerprint）
    for t in ["ap_invoice_vouchers", "ic_vouchers"] {
        if table_exists(conn, t)? && !has_column(conn, t, "cost_center")? {
            conn.execute_batch(&format!("ALTER TABLE {t} ADD COLUMN cost_center TEXT;"))?;
        }
    }
    Ok(())
}

/// v2 的人工结论表按 detail_id 全局唯一；新模型需要保留各所属期自己的人工结论。
/// 仅重建两张覆盖表，逐行保留旧数据与主键，不触碰命中/确认数据。
fn migrate_overrides_per_run(conn: &Connection) -> SqlResult<()> {
    for (table, create_sql, columns) in [
        (
            "welfare_transfer_overrides",
            "CREATE TABLE welfare_transfer_overrides_v3 (
                id INTEGER PRIMARY KEY, run_id INTEGER NOT NULL, detail_id INTEGER NOT NULL,
                included INTEGER CHECK (included IN (0,1)), transfer_tax INTEGER CHECK (transfer_tax IS NULL OR transfer_tax >= 0),
                remark TEXT, updated_by TEXT NOT NULL, updated_at TEXT NOT NULL,
                FOREIGN KEY (run_id) REFERENCES tax_runs(id), FOREIGN KEY (detail_id) REFERENCES invoice_details(id),
                UNIQUE (run_id, detail_id),
                CHECK (included IS NOT NULL OR transfer_tax IS NOT NULL OR (remark IS NOT NULL AND length(trim(remark)) > 0))
            );",
            "id,run_id,detail_id,included,transfer_tax,remark,updated_by,updated_at",
        ),
        (
            "stamp_tax_overrides",
            "CREATE TABLE stamp_tax_overrides_v3 (
                id INTEGER PRIMARY KEY, run_id INTEGER NOT NULL, detail_id INTEGER NOT NULL,
                included INTEGER CHECK (included IN (0,1)), stamp_tax_category_code TEXT,
                remark TEXT, updated_by TEXT NOT NULL, updated_at TEXT NOT NULL,
                FOREIGN KEY (run_id) REFERENCES tax_runs(id), FOREIGN KEY (detail_id) REFERENCES invoice_details(id),
                FOREIGN KEY (stamp_tax_category_code) REFERENCES stamp_tax_categories(code),
                UNIQUE (run_id, detail_id),
                CHECK (included IS NOT NULL OR stamp_tax_category_code IS NOT NULL OR (remark IS NOT NULL AND length(trim(remark)) > 0))
            );",
            "id,run_id,detail_id,included,stamp_tax_category_code,remark,updated_by,updated_at",
        ),
    ] {
        if !table_exists(conn, table)? { continue; }
        let ddl: String = conn.query_row(
            "SELECT sql FROM sqlite_master WHERE type='table' AND name=?1", [table], |r| r.get(0)
        )?;
        let compact: String = ddl.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_ascii_lowercase();
        if compact.contains("unique(run_id,detail_id)") { continue; }
        if !compact.contains("unique(detail_id)") { continue; }
        let temp = format!("{table}_v3");
        let migration = conn.execute_batch(&format!(
            "BEGIN IMMEDIATE;
             DROP TABLE IF EXISTS {temp}; {create_sql}
             INSERT INTO {temp} ({columns}) SELECT {columns} FROM {table};
             DROP TABLE {table}; ALTER TABLE {temp} RENAME TO {table};
             COMMIT;"
        ));
        if let Err(e) = migration {
            let _ = conn.execute_batch("ROLLBACK;");
            return Err(e);
        }
    }
    Ok(())
}

/// 在 init_db 中、CREATE_TAX_MATCH_TABLES 之后调用。
pub fn init_tax_workflow(conn: &Connection) -> SqlResult<()> {
    migrate_legacy(conn)?;
    migrate_tax_runs_source_reuse(conn)?;
    conn.execute_batch(CREATE_APP_ADMINS_AND_SETTINGS)?;
    conn.execute_batch(CREATE_TAX_RUNS)?;
    migrate_overrides_per_run(conn)?;
    // 兼容旧流程：把旧来源期间转换为所属期批次的一个来源范围。
    conn.execute("INSERT OR IGNORE INTO tax_run_sources (run_id, src_period_from, src_period_to, added_by, added_at) SELECT id, src_period_from, src_period_to, created_by, created_at FROM tax_runs", [])?;
    conn.execute_batch(CREATE_VOUCHER_VIEW)?;
    conn.execute_batch(CREATE_WELFARE_TRANSFER_TABLES)?;
    conn.execute_batch(CREATE_STAMP_TAX_TABLES)?;
    Ok(())
}
