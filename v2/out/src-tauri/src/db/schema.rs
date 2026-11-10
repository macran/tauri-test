// src-tauri/src/db/schema.rs
//
// 所有建表语句和初始化逻辑。
// init_db() 在应用启动时调用一次。

use rusqlite::{Connection, Result as SqlResult};

// ─────────────────────────────────────────────────────────────
// sellers 表
// 销方信息表。发票头通过 seller_id 关联。
// ─────────────────────────────────────────────────────────────
const CREATE_SELLERS: &str = "
CREATE TABLE IF NOT EXISTS sellers (
    id      INTEGER PRIMARY KEY,
    tax_no  TEXT    NOT NULL UNIQUE,  -- 销方纳税人识别号，全局唯一
    name    TEXT    NOT NULL          -- 销方名称，可更新
);
";

// ─────────────────────────────────────────────────────────────
// invoice_headers 表
// 发票头表。每行对应一张发票。
// 设计要点：
//   - inv_unq_key 为生成列，由数据库自动计算，应用层不插入
//   - 金额字段全部以分为单位存储（INTEGER）
//   - 枚举字段存储 u8 整数
//   - 预留红冲关联字段：reversed_by_id（将来做红冲匹配核算用）
// ─────────────────────────────────────────────────────────────
const CREATE_INVOICE_HEADERS: &str = "
CREATE TABLE IF NOT EXISTS invoice_headers (
    id                       INTEGER PRIMARY KEY,
    code                     TEXT,
    num                      TEXT,
    e_num                    TEXT,
    seller_id                INTEGER NOT NULL,
    issue_date               TEXT    NOT NULL,
    amount                   INTEGER NOT NULL,
    tax                      INTEGER NOT NULL,
    total                    INTEGER NOT NULL,
    source                   INTEGER NOT NULL,
    inv_type                 INTEGER NOT NULL,
    state                    INTEGER NOT NULL,
    is_pos                   INTEGER NOT NULL,
    risk                     INTEGER NOT NULL,
    issuer                   TEXT,
    remark                   TEXT,
    rev_org_key TEXT,
    red_notice_no            TEXT,
    inv_unq_key              TEXT GENERATED ALWAYS AS (
        CASE
            WHEN code IS NOT NULL AND TRIM(code) <> ''
             AND num  IS NOT NULL AND TRIM(num)  <> ''
            THEN TRIM(code) || '-' || TRIM(num)
            ELSE TRIM(e_num)
        END
    ) STORED NOT NULL,
    UNIQUE(inv_unq_key),
    FOREIGN KEY (seller_id) REFERENCES sellers(id)
);
CREATE VIRTUAL TABLE IF NOT EXISTS invoice_headers_fts USING fts5(
    seller_name,
    remark,
    content='',
    contentless_delete=1,
    tokenize='simple'
);
-- select * from invoice_headers_fts where seller_name match jieba_query('滴滴') limit 10;
-- INSERT 触发器保持不变（拼写修正：Iinvoice → invoice）
CREATE TRIGGER IF NOT EXISTS invoice_headers_ai
AFTER INSERT ON invoice_headers
BEGIN
    INSERT INTO invoice_headers_fts(
        rowid,
        seller_name,
        remark
    )
    SELECT
        NEW.id,
        s.name,
        COALESCE(NEW.remark, '')
    FROM sellers AS s
    WHERE s.id = NEW.seller_id;
END;

-- UPDATE 触发器：只监听 seller_id 变化（inv_unq_key 是生成列不可直接改）
CREATE TRIGGER IF NOT EXISTS invoice_headers_au
AFTER UPDATE OF seller_id, remark ON invoice_headers
WHEN OLD.seller_id IS NOT NEW.seller_id
   OR OLD.remark IS NOT NEW.remark
BEGIN
    UPDATE invoice_headers_fts
    SET
        seller_name = (
            SELECT s.name
            FROM sellers AS s
            WHERE s.id = NEW.seller_id
        ),
        remark = COALESCE(NEW.remark, '')
    WHERE rowid = NEW.id;
END;

-- DELETE 触发器保持不变
CREATE TRIGGER IF NOT EXISTS invoice_headers_ad
AFTER DELETE ON invoice_headers
BEGIN
    DELETE FROM invoice_headers_fts
    WHERE rowid = OLD.id;
END;

-- sellers UPDATE 触发器保持不变
CREATE TRIGGER IF NOT EXISTS sellers_au
AFTER UPDATE OF name ON sellers
WHEN OLD.name IS NOT NEW.name
BEGIN
    UPDATE invoice_headers_fts
    SET
        seller_name = NEW.name,
        remark = COALESCE(
            (
                SELECT h.remark
                FROM invoice_headers AS h
                WHERE h.id = invoice_headers_fts.rowid
            ),
            ''
        )
    WHERE rowid IN (
        SELECT h.id
        FROM invoice_headers AS h
        WHERE h.seller_id = NEW.id
    );
END;


CREATE INDEX IF NOT EXISTS idx_invoice_headers_seller_id
    ON invoice_headers(seller_id);
CREATE INDEX IF NOT EXISTS idx_invoice_headers_issue_date
    ON invoice_headers(issue_date);
CREATE INDEX IF NOT EXISTS idx_invoice_headers_state
    ON invoice_headers(state);

CREATE INDEX IF NOT EXISTS idx_invoice_headers_rev_org_key
    ON invoice_headers(rev_org_key) WHERE rev_org_key IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_invoice_headers_seller_id_issue_date
    ON invoice_headers(seller_id, issue_date);
";

// ─────────────────────────────────────────────────────────────
// invoice_details 表
// 发票明细表。每行对应一张发票的一个明细行。
// 设计要点：
//   - 通过 header_id 外键关联到 invoice_headers
//   - 金额字段全部以分为单位存储（INTEGER）
//   - special_biz 存储 u8 整数，对应 SpecialBizType 枚举
//   - 数量、单价保留 REAL（不转分，因为可能有小数）
// ─────────────────────────────────────────────────────────────
const CREATE_INVOICE_DETAILS: &str = "
CREATE TABLE IF NOT EXISTS invoice_details (
    id             INTEGER PRIMARY KEY,
    header_id      INTEGER NOT NULL,
    line_no        INTEGER NOT NULL,
    tax_code       TEXT,
    biz_type       INTEGER,
    goods_category TEXT,
    goods_name     TEXT    NOT NULL,
    spec_model     TEXT,
    unit           TEXT,
    quantity       TEXT,
    unit_price     TEXT,
    amount         INTEGER NOT NULL,
    tax_rate       TEXT,
    tax            INTEGER NOT NULL,
    total          INTEGER NOT NULL,
    UNIQUE(header_id, line_no),
    FOREIGN KEY (header_id) REFERENCES invoice_headers(id)
);

CREATE INDEX IF NOT EXISTS idx_invoice_details_biz_type
    ON invoice_details(biz_type);
CREATE INDEX IF NOT EXISTS idx_invoice_details_goods_category
    ON invoice_details(goods_category);
CREATE INDEX IF NOT EXISTS idx_invoice_details_header_id
    ON invoice_details(header_id);
CREATE INDEX IF NOT EXISTS idx_invoice_details_tax_code
    ON invoice_details(tax_code);
";

// ─────────────────────────────────────────────────────────────
// 红冲关联表（预留，将来红冲匹配核算时使用）
// 设计要点：
//   - 一张红字发票可以对应多张蓝字发票（部分红冲场景）
//   - amount_fen 记录本次红冲金额（分），支持部分红冲
// ─────────────────────────────────────────────────────────────

const CREATE_REVERSAL_RELATIONS: &str = "
CREATE TABLE IF NOT EXISTS reversal_line_matches (
    id                  INTEGER PRIMARY KEY,
    red_header_id       INTEGER NOT NULL,
    red_detail_id       INTEGER NOT NULL,
    blue_header_id      INTEGER,
    blue_detail_id      INTEGER,
    blue_discount_detail_id INTEGER,
    matched_quantity    REAL,
    matched_amount      INTEGER NOT NULL,
    matched_tax         INTEGER NOT NULL,
    matched_total       INTEGER NOT NULL,
    match_result_type   INTEGER NOT NULL DEFAULT 1,
    match_status        INTEGER NOT NULL DEFAULT 1
                             CHECK (match_status IN (1, 2)),
    match_note          TEXT,
    match_batch_no      INTEGER NOT NULL,
    
   -- UNIQUE(red_detail_id, blue_detail_id),

    FOREIGN KEY (red_header_id)  REFERENCES invoice_headers(id),
    FOREIGN KEY (red_detail_id)  REFERENCES invoice_details(id),
    FOREIGN KEY (blue_header_id) REFERENCES invoice_headers(id),
    FOREIGN KEY (blue_detail_id) REFERENCES invoice_details(id)
);
CREATE UNIQUE INDEX IF NOT EXISTS ux_rlm_red_blue
ON reversal_line_matches (
match_batch_no,
    red_detail_id,
    COALESCE(blue_detail_id, -1)
);
CREATE INDEX IF NOT EXISTS idx_rlm_blue_header_id  ON reversal_line_matches(blue_header_id);
CREATE INDEX IF NOT EXISTS idx_rlm_blue_detail_id  ON reversal_line_matches(blue_detail_id);
CREATE INDEX IF NOT EXISTS idx_rlm_red_header_id   ON reversal_line_matches(red_header_id);
CREATE INDEX IF NOT EXISTS idx_rlm_red_detail_id   ON reversal_line_matches(red_detail_id);
CREATE INDEX IF NOT EXISTS idx_rlm_match_batch_no  ON reversal_line_matches(match_batch_no);
CREATE INDEX IF NOT EXISTS idx_rlm_external_history
    ON reversal_line_matches(red_detail_id, match_status)
    WHERE match_status = 2;
";

const CREATE_SPECIAL_DETAILS: &str = "
CREATE TABLE IF NOT EXISTS building_details (
    id                        INTEGER PRIMARY KEY,
    header_id                 INTEGER NOT NULL,
    line_no                   INTEGER NOT NULL,
    cross_region_no           TEXT,
    service_location          TEXT,
    project_name              TEXT,
    UNIQUE(header_id, line_no),
    FOREIGN KEY (header_id) REFERENCES invoice_headers(id)
);

CREATE TABLE IF NOT EXISTS real_estate_details (
    id               INTEGER PRIMARY KEY,
    header_id        INTEGER NOT NULL,
    line_no          INTEGER NOT NULL,
    cert_no TEXT,
    plate_no         TEXT,
    UNIQUE(header_id, line_no),
    FOREIGN KEY (header_id) REFERENCES invoice_headers(id)
);

CREATE TABLE IF NOT EXISTS passenger_details (
    id              INTEGER PRIMARY KEY,
    header_id       INTEGER NOT NULL,
    line_no         INTEGER NOT NULL,
    passenger_name  TEXT,
    id_no           TEXT,
    travel_date     TEXT,
    depart          TEXT,
    arrival         TEXT,
    seat_class      TEXT,
    transport_type  TEXT,
    UNIQUE(header_id, line_no),
    FOREIGN KEY (header_id) REFERENCES invoice_headers(id)
);

CREATE TABLE IF NOT EXISTS cargo_details (
    id               INTEGER PRIMARY KEY,
    header_id        INTEGER NOT NULL,
    line_no          INTEGER NOT NULL,
    transport_type   TEXT,
    vehicle_no       TEXT,
    depart           TEXT,
    arrival          TEXT,
    cargo_name       TEXT,
    UNIQUE(header_id, line_no),
    FOREIGN KEY (header_id) REFERENCES invoice_headers(id)
);

CREATE INDEX IF NOT EXISTS idx_cargo_details_vehicle_no
    ON cargo_details(vehicle_no);

CREATE TABLE IF NOT EXISTS air_details (
    id              INTEGER PRIMARY KEY,
    header_id       INTEGER NOT NULL,
    line_no         INTEGER NOT NULL,
    passenger_name  TEXT,
    id_no           TEXT,
    depart          TEXT,
    arrival         TEXT,
    carrier         TEXT,
    flight_no       TEXT,
    flight_date     TEXT,
    flight_time     TEXT,
    UNIQUE(header_id, line_no),
    FOREIGN KEY (header_id) REFERENCES invoice_headers(id)
);

CREATE TABLE IF NOT EXISTS railway_details (
    id              INTEGER PRIMARY KEY,
    header_id       INTEGER NOT NULL,
    line_no         INTEGER NOT NULL,
    passenger_name  TEXT,
    id_no           TEXT,
    depart          TEXT,
    arrival         TEXT,
    train_no        TEXT,
    travel_date     TEXT,
    travel_time     TEXT,
    UNIQUE(header_id, line_no),
    FOREIGN KEY (header_id) REFERENCES invoice_headers(id)
);

CREATE TABLE IF NOT EXISTS toll_details (
    id              INTEGER PRIMARY KEY,
    header_id       INTEGER NOT NULL,
    line_no         INTEGER NOT NULL,
    plate_no        TEXT,
    vehicle_type    TEXT,
    toll_date_from TEXT,
    toll_date_to   TEXT,
    UNIQUE(header_id, line_no),
    FOREIGN KEY (header_id) REFERENCES invoice_headers(id)
);

CREATE INDEX IF NOT EXISTS idx_toll_details_plate_no
    ON toll_details(plate_no);

";

//-------认证清单表 整合了T&E 表-  另外 勾选状态是固定的忽略掉，sql查询的时候增加"已勾选"三个字------
const CREATE_CERTIFIED_LIST: &str = "
CREATE TABLE IF NOT EXISTS certified_list (
    id                      INTEGER PRIMARY KEY,
    header_id               INTEGER NOT NULL UNIQUE,
    domestic_proof_no       TEXT,
    valid_deduct_amount     INTEGER NOT NULL,
    invoice_tag             TEXT    NOT NULL,
    check_time              TEXT    NOT NULL,
    risk_status             INTEGER NOT NULL,
    tax_period              TEXT,
    is_travel_expense       INTEGER DEFAULT 0 CHECK (is_travel_expense IN (0, 1)),
    voucher_no              TEXT,
    posted_date            TEXT,

    FOREIGN KEY (header_id) REFERENCES invoice_headers(id)
);

CREATE INDEX IF NOT EXISTS idx_certified_list_voucher_no
    ON certified_list(voucher_no);
CREATE INDEX IF NOT EXISTS idx_certified_list_tax_period
    ON certified_list(tax_period);
";

const CREATE_BANK_INVOICE_VOUCHERS: &str = "
CREATE TABLE IF NOT EXISTS bank_invoice_vouchers (
    id             INTEGER PRIMARY KEY,
    header_id      INTEGER NOT NULL UNIQUE,
    voucher_no     TEXT    NOT NULL,
    posted_date    TEXT    NOT NULL,
    received_date  TEXT    NOT NULL,
    prepared_by    TEXT   ,

    FOREIGN KEY (header_id) REFERENCES invoice_headers(id)
);

CREATE INDEX IF NOT EXISTS idx_bank_invoice_vouchers_header_id
    ON bank_invoice_vouchers(header_id);

CREATE INDEX IF NOT EXISTS idx_bank_invoice_vouchers_voucher_no
    ON bank_invoice_vouchers(voucher_no);

CREATE INDEX IF NOT EXISTS idx_bank_invoice_vouchers_posted_date
    ON bank_invoice_vouchers(posted_date);
";


// ─────────────────────────────────────────────────────────────
// vendors 表
// AP 供应商信息表。ap_invoice_vouchers 通过 vendor_id 关联。
// vendor_code 是SAP 中的供应商编码，vendor_name 是供应商名称。
// 因为发票上的名称和SAP中的名称可能不一致，所以除了seller表之外再建一个供应商表，方便后续做供应商匹配和核算。
// ─────────────────────────────────────────────────────────────
const CREATE_VENDORS: &str = "
CREATE TABLE IF NOT EXISTS vendors (
    id           INTEGER PRIMARY KEY,
    vendor_code  TEXT    NOT NULL UNIQUE,  -- 供应商编码，全局唯一
    vendor_name  TEXT    NOT NULL          -- 供应商名称
);
CREATE INDEX IF NOT EXISTS idx_vendors_vendor_code
    ON vendors(vendor_code);
";

// ─────────────────────────────────────────────────────────────
// ap_invoice_vouchers 表
// AP 应付凭证表。每行对应一条 AP 凭证记录。
// 设计要点：
//   - header_id 可为 NULL（收据、票据、通知单等非标准发票）
//   - vendor_id 关联 vendors 表
//   - 金额字段全部以分为单位存储（INTEGER）
//   - 税率存 TEXT（如 "13%"）
//   - inv_type 存 u8 整数，对应 ApInvType 枚举
//   - tax_certified 存 u8 整数，对应 TaxCertified 枚举
//   - received_date / preparer 由前端用户输入，NOT NULL
//
// 判重设计（content_fingerprint）：
//   barcode 历史数据太随意（同一邮件多张发票共用一个 barcode、个别为空），
//   不适合做唯一约束。改用 content_fingerprint：对「业务身份」字段
//   （header_id 不为空时用 header_id，否则退化为 barcode；
//    再加 vendor_id / amount / tax / total / tax_rate / inv_type / po_no /
//    invoice_date）做 sha256，UNIQUE(content_fingerprint)。
//   故意不含 voucher_no、entry_no、entry_date、remark、tax_certified、
//   received_date、preparer —— 发票红冲后重新做账，凭证号/入账日期会变，
//   但业务身份不变，这几个字段改为导入时 UPSERT 覆盖，而不是生成新行。
//   同一发票因 item 税率不同拆成多行时，tax_rate/amount/tax 不同，
//   fingerprint 自然不同，两行都会被正常保留。
// ─────────────────────────────────────────────────────────────
const CREATE_AP_INVOICE_VOUCHERS: &str = "
CREATE TABLE IF NOT EXISTS ap_invoice_vouchers (
    id                  INTEGER PRIMARY KEY,
    header_id           INTEGER,              -- 可为 NULL
    vendor_id           INTEGER NOT NULL,
    barcode             TEXT,                 -- 不作唯一约束，仅展示/排查用
    invoice_no_display  TEXT    NOT NULL,     -- 原始发票号（旧票=代码+号码，数电票=号码）；
                                               -- 收据/通知单等无 header_id 时用 barcode+凭证号构造
    entry_no            TEXT,                 -- 入账号码
    amount              INTEGER NOT NULL,     -- 不含税金额（分）
    tax_rate            TEXT,                 -- 税率，如 \"13%\"
    tax                 INTEGER NOT NULL,     -- 税额（分）
    total               INTEGER NOT NULL,     -- 含税金额（分）
    inv_type            INTEGER NOT NULL,     -- ApInvType 枚举
    po_no               TEXT,                 -- PO 号
    voucher_no          TEXT    NOT NULL,     -- 凭证号，业务上保证非空
    invoice_date        TEXT,                 -- 开票日期
    entry_date          TEXT,                 -- 入账日期
    remark              TEXT,                 -- 备注
    tax_certified       INTEGER NOT NULL,     -- TaxCertified 枚举
    received_date       TEXT    NOT NULL,     -- 接收日期（前端输入）
    preparer            TEXT    NOT NULL,     -- 制表人（前端输入）

    content_fingerprint BLOB    NOT NULL
                        CHECK (typeof(content_fingerprint) = 'blob'
                               AND length(content_fingerprint) = 32),

    FOREIGN KEY (header_id) REFERENCES invoice_headers(id),
    FOREIGN KEY (vendor_id) REFERENCES vendors(id)
);
CREATE UNIQUE INDEX IF NOT EXISTS ux_ap_vouchers_fingerprint
    ON ap_invoice_vouchers(content_fingerprint);
CREATE INDEX IF NOT EXISTS idx_ap_vouchers_header_id
    ON ap_invoice_vouchers(header_id) WHERE header_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_ap_vouchers_vendor_id
    ON ap_invoice_vouchers(vendor_id);
CREATE INDEX IF NOT EXISTS idx_ap_vouchers_entry_date
    ON ap_invoice_vouchers(entry_date);
CREATE INDEX IF NOT EXISTS idx_ap_vouchers_voucher_no
    ON ap_invoice_vouchers(voucher_no);
CREATE INDEX IF NOT EXISTS idx_ap_vouchers_barcode
    ON ap_invoice_vouchers(barcode) WHERE barcode IS NOT NULL;
";

// ─────────────────────────────────────────────────────────────
// user_layouts 表
// 查询结果表格的列布局（列顺序/显隐/列宽）。
//   scope      表格标识（如 invoice-header）
//   owner      创建者的 Windows 用户名（大写，同 AppRuntimeState.username）。
//              数据库文件会同步到共享盘，多人共用，所以需要 owner
//   is_private 0 = 公开（所有人可用），1 = 仅 owner 可见；默认公开
//   is_default 1 = owner 把它设为自己在该 scope 的默认布局（查询时自动选中）；
//              每个 (scope, owner) 至多一条。系统默认布局由前端按列定义生成，不落库
//   layout     前端序列化的 JSON
// ─────────────────────────────────────────────────────────────
pub(crate) const CREATE_USER_LAYOUTS: &str = "
CREATE TABLE IF NOT EXISTS user_layouts (
    id          INTEGER PRIMARY KEY,
    scope       TEXT    NOT NULL,
    owner       TEXT    NOT NULL,
    name        TEXT    NOT NULL,
    layout      TEXT    NOT NULL,
    is_default  INTEGER NOT NULL DEFAULT 0 CHECK (is_default IN (0, 1)),
    is_private  INTEGER NOT NULL DEFAULT 0 CHECK (is_private IN (0, 1)),
    updated_at  TEXT    NOT NULL DEFAULT (datetime('now', 'localtime')),
    UNIQUE(scope, owner, name)
);
CREATE UNIQUE INDEX IF NOT EXISTS ux_user_layouts_default
    ON user_layouts(scope, owner) WHERE is_default = 1;
";

const CREATE_IC_VOUCHERS: &str = "
CREATE TABLE IF NOT EXISTS ic_vouchers (
    id              INTEGER PRIMARY KEY,
    header_id       INTEGER NOT NULL,
    legal_entity    TEXT    NOT NULL,
    billing         TEXT,
    document_no     TEXT    NOT NULL,
    posted_date     TEXT,
    original_po     TEXT,
    received_date   TEXT    NOT NULL,
    preparer        TEXT    NOT NULL,
    UNIQUE(header_id, document_no),
    FOREIGN KEY (header_id) REFERENCES invoice_headers(id)
);

CREATE INDEX IF NOT EXISTS idx_ic_vouchers_header_id
    ON ic_vouchers(header_id);
CREATE INDEX IF NOT EXISTS idx_ic_vouchers_document_no
    ON ic_vouchers(document_no);
";


const CREATE_TAX_MATCH_TABLES: &str = r#"


-- ============================================================
-- 1. 印花税类别及税率表
-- ============================================================
CREATE TABLE IF NOT EXISTS stamp_tax_categories (
    code          TEXT    NOT NULL PRIMARY KEY,
    name          TEXT    NOT NULL,
    parent_code   TEXT    REFERENCES stamp_tax_categories(code) ON DELETE RESTRICT,
    rate_ppm      INTEGER CHECK (rate_ppm IS NULL OR rate_ppm > 0),  -- NULL = 必须选子目
    
    CHECK (
        (instr(code, 'YH') = 0 AND parent_code IS NULL)
        OR
        (instr(code, 'YH') > 1 AND parent_code = substr(code, 1, instr(code, 'YH') - 1))
    )
) STRICT;

CREATE INDEX IF NOT EXISTS idx_stamp_tax_categories_parent ON stamp_tax_categories(parent_code);


-- ============================================================
-- 2. 税务匹配规则表
--
-- 一行 = 一条完整规则
-- 同一行中填写的多个匹配条件 = AND
-- 不同规则之间独立判断
-- ============================================================

CREATE TABLE IF NOT EXISTS tax_match_rules (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    scope TEXT NOT NULL
        CHECK (scope IN ('input_tax_transfer', 'stamp_tax')),
    rule_category TEXT NOT NULL
        CHECK (length(trim(rule_category)) > 0),

    stamp_tax_category_code TEXT,

    seller_tax_id_match_type TEXT
        CHECK (seller_tax_id_match_type IN ('eq', 'neq')
            OR seller_tax_id_match_type IS NULL),
    seller_tax_id_values TEXT,

    tax_code_match_type TEXT
        CHECK (tax_code_match_type IN ('eq', 'neq')
            OR tax_code_match_type IS NULL),
    tax_code_values TEXT,

    tax_name_match_type TEXT
        CHECK (tax_name_match_type IN ('eq', 'neq')
            OR tax_name_match_type IS NULL),
    tax_name_values TEXT,

    needs_manual_review INTEGER NOT NULL DEFAULT 0
        CHECK (needs_manual_review IN (0, 1)),
    note TEXT NOT NULL DEFAULT '',

    enabled INTEGER NOT NULL DEFAULT 1
        CHECK (enabled IN (0, 1)),
    disabled_at TEXT,

    source TEXT NOT NULL
        CHECK (source IN ('excel', 'manual')),
    created_at TEXT NOT NULL,
    rule_signature BLOB NOT NULL
    CHECK (
        typeof(rule_signature) = 'blob'
        AND length(rule_signature) = 32
    ),

    FOREIGN KEY (stamp_tax_category_code)
        REFERENCES stamp_tax_categories(code)
        ON UPDATE RESTRICT
        ON DELETE RESTRICT,

    CHECK (
        (scope = 'stamp_tax' AND stamp_tax_category_code IS NOT NULL)
        OR
        (scope = 'input_tax_transfer' AND stamp_tax_category_code IS NULL)
    ),
    CHECK (
        (seller_tax_id_match_type IS NULL AND seller_tax_id_values IS NULL)
        OR
        (seller_tax_id_match_type IS NOT NULL
            AND seller_tax_id_values IS NOT NULL
            AND length(trim(seller_tax_id_values)) > 0)
    ),
    CHECK (
        (tax_code_match_type IS NULL AND tax_code_values IS NULL)
        OR
        (tax_code_match_type IS NOT NULL
            AND tax_code_values IS NOT NULL
            AND length(trim(tax_code_values)) > 0)
    ),
    CHECK (
        (tax_name_match_type IS NULL AND tax_name_values IS NULL)
        OR
        (tax_name_match_type IS NOT NULL
            AND tax_name_values IS NOT NULL
            AND length(trim(tax_name_values)) > 0)
    ),
    CHECK (
        seller_tax_id_match_type IS NOT NULL
        OR tax_code_match_type IS NOT NULL
        OR tax_name_match_type IS NOT NULL
    ),
    CHECK (
        (enabled = 1 AND disabled_at IS NULL)
        OR
        (enabled = 0 AND disabled_at IS NOT NULL)
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS
    ux_tax_match_rules_active_signature
ON tax_match_rules(rule_signature)
WHERE enabled = 1;

CREATE INDEX IF NOT EXISTS ix_tax_match_rules_active_scope_category
ON tax_match_rules(scope, rule_category, enabled);

CREATE INDEX IF NOT EXISTS ix_tax_match_rules_active_stamp_category
ON tax_match_rules(stamp_tax_category_code, enabled)
WHERE scope = 'stamp_tax' AND enabled = 1;

CREATE INDEX IF NOT EXISTS ix_tax_match_rules_active_seller_tax_id
ON tax_match_rules(seller_tax_id_match_type, seller_tax_id_values, enabled)
WHERE enabled = 1;

CREATE INDEX IF NOT EXISTS ix_tax_match_rules_active_tax_code
ON tax_match_rules(tax_code_match_type, tax_code_values, enabled)
WHERE enabled = 1;

CREATE INDEX IF NOT EXISTS ix_tax_match_rules_active_tax_name
ON tax_match_rules(tax_name_match_type, tax_name_values, enabled)
WHERE enabled = 1;
"#;

// ─────────────────────────────────────────────────────────────
// const CREATE_TAX_CATEGORY: &str = "
// CREATE TABLE IF NOT EXISTS tax_category (
//     id INTEGER PRIMARY KEY,
//     code TEXT NOT NULL UNIQUE,
//     name TEXT NOT NULL,
//     short_name TEXT NOT NULL DEFAULT ''
// );

// CREATE INDEX IF NOT EXISTS idx_tax_category_name
// ON tax_category(name);

// CREATE INDEX IF NOT EXISTS idx_tax_category_short_name
// ON tax_category(short_name);

// ─────────────────────────────────────────────────────────────
// seed 记录表：记录哪些初始数据已经执行过，避免重复插入
// ─────────────────────────────────────────────────────────────
// const CREATE_SEED_HISTORY: &str = "
// CREATE TABLE IF NOT EXISTS _seed_history (
//     name         TEXT PRIMARY KEY,
//     executed_at  TEXT NOT NULL
// );
// ";
const STAMP_CATEGORY_DATA_SQL: &str =
     include_str!("../../seed/stamp_category.sql");
// ─────────────────────────────────────────────────────────────
// init_db
// 应用启动时调用一次。建表建索引均使用 IF NOT EXISTS，安全重入。
// ─────────────────────────────────────────────────────────────
pub fn init_db(conn: &mut Connection) -> SqlResult<()> {
    // WAL 模式：提升并发读写性能
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
             PRAGMA synchronous  = NORMAL;
             PRAGMA cache_size   = -64000;
             PRAGMA temp_store   = MEMORY;
             PRAGMA mmap_size    = 268435456;
             PRAGMA foreign_keys = ON;
             PRAGMA busy_timeout = 5000;",
    )?;
    // 开启外键约束
    // conn.execute_batch("PRAGMA foreign_keys = ON;")?;
    // 提升写入性能
    //conn.execute_batch("PRAGMA synchronous = NORMAL;")?;

    // 建表（顺序不能换：sellers 必须先于 invoice_headers）
    conn.execute_batch(CREATE_SELLERS)?;
    conn.execute_batch(CREATE_INVOICE_HEADERS)?;
    conn.execute_batch(CREATE_INVOICE_DETAILS)?;
    conn.execute_batch(CREATE_REVERSAL_RELATIONS)?;
    conn.execute_batch(CREATE_SPECIAL_DETAILS)?;
    conn.execute_batch(CREATE_CERTIFIED_LIST)?;
    conn.execute_batch(CREATE_BANK_INVOICE_VOUCHERS)?;
    conn.execute_batch(CREATE_VENDORS)?;
    // ap_invoice_vouchers 的唯一约束从 (无) 改为 content_fingerprint，
    // 旧表缺少该列，CREATE TABLE IF NOT EXISTS 不会迁移，需先清掉。
    // drop_table_if_stale(conn, "ap_invoice_vouchers", |c, t| {
    //     Ok(!has_column(c, t, "content_fingerprint")?)
    // })?;
    conn.execute_batch(CREATE_AP_INVOICE_VOUCHERS)?;
    conn.execute_batch(CREATE_IC_VOUCHERS)?;
    conn.execute_batch(CREATE_USER_LAYOUTS)?;
    // add_ic_vouchers_posted_date_column(conn)?;
    // conn.execute_batch(CREATE_TAX_CATEGORY)?;
    // tax_match_rules / welfare_transfer_rule_hits / welfare_transfer_results 的表结构
    // 在开发过程中整体重做过，CREATE TABLE IF NOT EXISTS 不会迁移旧表，必须先清掉
    // 残留的旧结构表，否则随后建索引时会报 "no such column"。
    // drop_stale_rule_engine_tables(conn)?;
    // CREATE_TAX_MATCH_TABLES 建立 tax_match_rules，必须先于
    // CREATE_WELFARE_TRANSFER_TABLES（welfare_transfer_rule_hits 外键引用 tax_match_rules）
    conn.execute_batch(CREATE_TAX_MATCH_TABLES)?;
    // 进项税转出 / 印花税：纳税所属期结果集、凭证批次、来源期间、确认快照、凭证视图
    // （取代旧的 tax_period_closings / tax_period_confirmations 与按 tax_period 存储的结果表）
    super::schema_tax_workflow::init_tax_workflow(conn)?;
    // conn.execute_batch(CREATE_SEED_HISTORY)?;
    // drop_tax_match_rules_enabled_at(conn)?;
    init_stamp_category(conn)?;
    Ok(())
}

// 判断某张表是否为旧版遗留结构：is_stale(table) 返回 true 时才会被 DROP。
// 仅在表为空时才 DROP（避免误删真实数据），否则报错提示人工处理。
// fn drop_table_if_stale(
//     conn: &Connection,
//     table: &str,
//     is_stale: impl FnOnce(&Connection, &str) -> SqlResult<bool>,
// ) -> SqlResult<()> {
//     let table_exists: bool = conn
//         .prepare("SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1")?
//         .exists([table])?;
//     if !table_exists || !is_stale(conn, table)? {
//         return Ok(());
//     }

//     let row_count: i64 =
//         conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| row.get(0))?;
//     if row_count > 0 {
//         return Err(rusqlite::Error::SqliteFailure(
//             rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_SCHEMA),
//             Some(format!(
//                 "表 {table} 的结构已过期，且包含 {row_count} 行数据，\
//                  无法自动迁移，请手动备份/处理后再启动"
//             )),
//         ));
//     }

//     conn.execute_batch(&format!("DROP TABLE {table};"))?;
//     Ok(())
// }

// fn has_column(conn: &Connection, table: &str, column: &str) -> SqlResult<bool> {
//     conn.prepare("SELECT 1 FROM pragma_table_info(?1) WHERE name = ?2")?
//         .exists(rusqlite::params![table, column])
// }

// // posted_date 是后来补充到 ic_vouchers 的列，早期建的库里没有，
// // CREATE TABLE IF NOT EXISTS 不会给已存在的表追加列，这里显式补齐。
// fn add_ic_vouchers_posted_date_column(conn: &Connection) -> SqlResult<()> {
//     if !has_column(conn, "ic_vouchers", "posted_date")? {
//         conn.execute_batch("ALTER TABLE ic_vouchers ADD COLUMN posted_date TEXT;")?;
//     }
//     Ok(())
// }

// fn drop_stale_rule_engine_tables(conn: &Connection) -> SqlResult<()> {
//     // 新表缺少 rule_signature 列 → 旧版 tax_match_rules 结构
//     drop_table_if_stale(conn, "tax_match_rules", |c, t| {
//         Ok(!has_column(c, t, "rule_signature")?)
//     })?;
//     // 旧版 welfare_transfer_rule_hits 多出 transfer_reason 列（新版只有 id/result_id/rule_id）
//     drop_table_if_stale(conn, "welfare_transfer_rule_hits", |c, t| {
//         has_column(c, t, "transfer_reason")
//     })?;
//     // 旧版 welfare_transfer_results 没有 period_from 列
//     drop_table_if_stale(conn, "welfare_transfer_results", |c, t| {
//         Ok(!has_column(c, t, "period_from")?)
//     })?;
//     // 旧版 stamp_tax_results 按 calc_batch_no 分批、没有 period_from 列
//     drop_table_if_stale(conn, "stamp_tax_results", |c, t| {
//         Ok(!has_column(c, t, "period_from")?)
//     })?;
//     // 旧版 stamp_tax_rule_hits 带 rule_code 快照列（新版只有 id/result_id/rule_id）
//     drop_table_if_stale(conn, "stamp_tax_rule_hits", |c, t| {
//         has_column(c, t, "rule_code")
//     })?;
//     Ok(())
// }

// // tax_match_rules.enabled_at 与 created_at 恒等（规则创建即生效，不允许编辑/删除），
// // 属于冗余列。CREATE TABLE IF NOT EXISTS 无法迁移已存在的库，这里显式检测并 DROP 一次。
// fn drop_tax_match_rules_enabled_at(conn: &Connection) -> SqlResult<()> {
//     let has_column: bool = conn
//         .prepare("SELECT 1 FROM pragma_table_info('tax_match_rules') WHERE name = 'enabled_at'")?
//         .exists([])?;

//     if has_column {
//         conn.execute_batch("ALTER TABLE tax_match_rules DROP COLUMN enabled_at;")?;
//     }

//     Ok(())
// }

fn init_stamp_category(conn: &mut Connection) -> SqlResult<()> {

    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM stamp_tax_categories",
        [],
        |row| row.get(0),
    )?;

    // 已经有数据，不重复导入
    if count > 0 {
        return Ok(());
    }

    let tx = conn.transaction()?;

    tx.execute_batch(STAMP_CATEGORY_DATA_SQL)?;

    tx.commit()?;

    Ok(())
}


// 执行所有 seed，每个 seed 仅首次运行一次 如果是多个seed文件可以使用这个方案。但目前我们只有一个seed文件。
// fn run_seeds(conn: &Connection) -> SqlResult<()> {
//     // 以后新增 seed 文件只需往这个数组里加一行
//     let seeds: &[(&str, &str)] = &[
//         (
//             "tax_category.sql",
//             include_str!("../../seed/tax_category.sql"),
//         ),
//         // ("other_table.sql", include_str!("../../seed/other_table.sql")),
//     ];

//     for (name, sql) in seeds {
//         run_seed_once(conn, name, sql)?;
//     }

//     Ok(())
// }

// fn run_seed_once(conn: &Connection, name: &str, sql: &str) -> SqlResult<()> {
//     let already_done: bool = conn.query_row(
//         "SELECT EXISTS(SELECT 1 FROM _seed_history WHERE name = ?1)",
//         [name],
//         |row| row.get(0),
//     )?;

//     if already_done {
//         return Ok(());
//     }

//     conn.execute_batch(sql)?;

//     conn.execute(
//         "INSERT INTO _seed_history (name, executed_at) VALUES (?1, datetime('now'))",
//         [name],
//     )?;

//     Ok(())
// }
