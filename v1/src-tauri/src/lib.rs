// src-tauri/src/lib.rs

use std::{
    fs,
    io,
    path::{Path, PathBuf},
    sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex},
};

use chrono::Datelike;

use rusqlite::{Connection, OpenFlags, OptionalExtension};

// mod bindings;
pub mod commands;
pub mod db;
pub mod error;

pub mod excel_reader;
pub mod models;
pub mod parser;
pub mod utils;
pub mod features;
mod startup;
use startup::{retry_startup, upload_local_to_shared, StartupState};
// use commands::import_refactored::import_invoice_excel;
use commands::import_ap_voucher::import_ap_voucher_excel;
use commands::import_bank::import_bank_voucher_excel;
use commands::import_certified::import_certified_excel;
use commands::import_ic_voucher::import_ic_voucher_excel;
use commands::import_invoice::import_invoice_excel;
use commands::import_travel_expense::import_travel_expense_excel;
use commands::match_invoice::match_reversal_invoices;
use commands::voucher_edit::{
    delete_ap_vouchers, delete_bank_vouchers, delete_ic_vouchers, update_ap_voucher,
    update_bank_voucher, update_ic_voucher,
};
use commands::admin::{
    admin_add_le, admin_add_user, admin_overview, admin_rename_le, admin_set_le_active,
    admin_update_user, current_user_info,
};
use commands::layout::{delete_layout, list_layouts, save_layout, set_default_layout};
use commands::query::{
    query_air_details, query_ap_vouchers, query_bank_vouchers, query_building_details,
    query_cargo_details, query_certified_list, query_ic_vouchers, query_invoice_details,
    query_invoice_headers, query_passenger_details, query_railway_details,
    query_real_estate_details, query_toll_details, query_travel_expense, query_item_overview,
};
use db::schema::init_db;
use specta_typescript::Typescript;
use tauri_specta::{collect_commands, collect_events, Builder};
use features::rules::rules_commands::{
    add_tax_match_rule,
    disable_tax_match_rule,
    import_tax_rules,
    list_tax_match_rules,
    list_stamp_tax_categories,
    
};

use features::input_tax_transfer::commands::{
    load_input_tax_transfer_review, run_input_tax_transfer_match,
    save_input_tax_transfer_override, save_input_tax_transfer_overrides_batch,
};
use features::stamp_tax::commands::{
    load_stamp_tax_review, run_stamp_tax_match, save_stamp_tax_override,
    save_stamp_tax_overrides_batch,
};
use tauri::AppHandle;
use tauri::Manager;

const LOCAL_DB_DIR: &str = "invoiceManage";
//const SHARED_ROOT: &str = r"\\Cnsh1cdot102.ap.merckgroup.com\SHA_WORKAREAS\Tax\InputTax";
const SHARED_ROOT: &str = r"\\cnbj1file01\Finance\01. Accounting\InputTax";
const ADMIN_DB_FILE: &str = "admin.db";
const DB_SIZE_LIMIT_BYTES: u64 = 5 * 1024 * 1024 * 1024; //5G 为大小。超过了就分库

// ────────────────────────────────────────────────────────────
// DbState
// 保留原有 DbState 结构，避免现有 commands 需要同步修改。
// ─────────────────────────────────────────────────────────────
pub struct DbState {
    pub conn: Arc<Mutex<Connection>>,
}

impl DbState {
    pub fn new(conn: Connection) -> Self {
        Self {
            conn: Arc::new(Mutex::new(conn)),
        }
    }
}

// ────────────────────────────────────────────────────────────
// 管理数据库读取结果
// admin.db 是共享盘上的权威配置；本地只保存启动时的缓存副本。
// ────────────────────────────────────────────────────────────
#[derive(Clone, Debug)]
struct AdminUser {
    username: String,
    full_name: String,
    default_le_code: Option<String>,
    is_leader: bool,
}

// 应用运行时状态。业务命令仍然使用 DbState；管理命令后续可使用这些字段。
#[derive(Clone)]
pub struct AppRuntimeState {
    pub username: String,
    pub full_name: String,
    pub default_le_code: Option<String>,
    pub active_le_codes: Vec<String>,
    pub active_le: String,
    pub dashboard_mode: bool,
    pub is_leader: bool,
    pub admin_db_path: PathBuf,
    pub active_db_path: PathBuf,
    pub local_root: PathBuf,
    pub shared_root: PathBuf,
}

fn detect_username() -> String {
    whoami::username().trim().to_ascii_uppercase()
}

// ─────────────────────────────────────────────────────────────
// run()
// ─────────────────────────────────────────────────────────────
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = <Builder>::new()
        .commands(collect_commands![
            import_certified_excel,
            import_invoice_excel,
            import_travel_expense_excel,
            import_bank_voucher_excel,
            import_ap_voucher_excel,
            import_ic_voucher_excel,
            match_reversal_invoices,
            retry_startup,
            upload_local_to_shared,

            query_invoice_headers,
            query_invoice_details,
            query_building_details,
            query_real_estate_details,
            query_passenger_details,
            query_cargo_details,
            query_air_details,
            query_railway_details,
            query_toll_details,
            query_certified_list,
            query_travel_expense,
            query_bank_vouchers,
            query_ap_vouchers,
            query_ic_vouchers,
            query_item_overview,
            current_user_info,
            admin_overview,
            admin_add_user,
            admin_update_user,
            admin_add_le,
            admin_set_le_active,
            admin_rename_le,

            list_layouts,
            save_layout,
            set_default_layout,
            delete_layout,
            update_bank_voucher,
            update_ap_voucher,
            update_ic_voucher,
            delete_bank_vouchers,
            delete_ap_vouchers,
            delete_ic_vouchers,

            import_tax_rules,
            add_tax_match_rule,
            disable_tax_match_rule,
            list_tax_match_rules,
            list_stamp_tax_categories,

            run_input_tax_transfer_match,
    load_input_tax_transfer_review,
    save_input_tax_transfer_override,
    save_input_tax_transfer_overrides_batch,

            run_stamp_tax_match,
            load_stamp_tax_review,
            save_stamp_tax_override,
            save_stamp_tax_overrides_batch,
        ])
        .events(collect_events![]);

    #[cfg(debug_assertions)]
    builder
        .export(Typescript::default(), "../src/bindings.ts")
        .expect("Failed to export typescript bindings");

    let backup_started = Arc::new(AtomicBool::new(false));
    let backup_started_for_run = Arc::clone(&backup_started);

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            builder.mount_events(app); // < Required for events to work
            app.manage(StartupState::new());
            // 数据库同步由 splash 窗口触发（retry_startup），成功后才创建主窗口。
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("构建 Tauri 应用时发生错误");

    app.run(move |app_handle, event| {
        if matches!(event, tauri::RunEvent::ExitRequested { .. })
            && !backup_started_for_run.swap(true, Ordering::SeqCst)
        {
            backup_on_exit(app_handle);
        }
    });
}

fn validate_admin_database(
    path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;

    let integrity: String = conn.query_row("PRAGMA integrity_check;", [], |row| row.get(0))?;
    if integrity != "ok" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("admin.db 完整性检查失败：{integrity}"),
        )
        .into());
    }

    let table_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master\n         WHERE type = 'table'\n           AND name IN ('app_users', 'legal_entities')",
        [],
        |row| row.get(0),
    )?;

    if table_count != 2 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "admin.db 缺少 app_users 或 legal_entities 表",
        )
        .into());
    }

    Ok(())
}

fn load_admin_context(
    admin_db_path: &Path,
    username: &str,
) -> Result<(AdminUser, Vec<String>), Box<dyn std::error::Error>> {
    let conn = Connection::open_with_flags(admin_db_path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;

    let user: Option<AdminUser> = conn
        .query_row(
            "SELECT username, full_name, default_le_code, is_active, is_leader\n             FROM app_users\n             WHERE username = ?1 COLLATE NOCASE",
            [username],
            |row| {
                let is_active: i64 = row.get(3)?;
                let is_leader: i64 = row.get(4)?;
                Ok((
                    AdminUser {
                        username: row.get(0)?,
                        full_name: row.get(1)?,
                        default_le_code: row.get(2)?,
                        is_leader: is_leader == 1,
                    },
                    is_active == 1,
                ))
            },
        )
        .optional()?
        .and_then(|(user, is_active)| is_active.then_some(user));

    let user = user.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("用户 {username} 不存在或已被停用"),
        )
    })?;

    let active_le_codes = {
        let mut statement = conn.prepare(
            "SELECT le_code\n             FROM legal_entities\n             WHERE is_active = 1\n             ORDER BY le_code",
        )?;
        let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
        rows.collect::<Result<Vec<_>, _>>()?
    };

    if let Some(default_le) = user.default_le_code.as_deref() {
        if !active_le_codes.iter().any(|le| le == default_le) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("用户 {} 的默认 LE {} 不存在或未激活", user.username, default_le),
            )
            .into());
        }
    }

    Ok((user, active_le_codes))
}

/// 确保某个 LE 的数据库在本地可用。
///
/// 这个函数的顺序不能改变：
/// 1. 调用前已由 startup 模块完成与共享盘的同步（失败会在启动页拦下）；
/// 2. 共享盘和本地都没有时创建新库；
/// 3. 选择年份最大的数据库作为当前数据库；
/// 4. 最新数据库达到 800 MB 时，创建新的年度数据库。
///
/// 后续实现 LE 切换时，应在切换前调用这个函数，而不是直接创建空数据库。
pub(crate) fn ensure_le_database(
    app: &AppHandle,
    local_root: &Path,
    le: &str,
    year: i32,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let mut databases = list_le_databases(local_root, le)?;

    if databases.is_empty() {
        let new_path = local_root.join(format!("{le}_{year}.db"));
        initialize_database_file(app, &new_path)?;
        return Ok(new_path);
    }

    databases.sort_by_key(|(_, file_year)| *file_year);
    let (latest_path, latest_year) = databases.last().expect("数据库列表不应为空");
    let latest_year = *latest_year;

    // 只有每年一月才检查是否需要按年度轮换数据库。
    // 同一年内即使数据库超过 800 MB，也继续使用当前数据库。
    let is_january = chrono::Local::now().month() == 1;
    let latest_db_is_from_previous_year = latest_year < year;
    let latest_db_is_too_large = fs::metadata(latest_path)?.len() >= DB_SIZE_LIMIT_BYTES;

    if is_january && latest_db_is_from_previous_year && latest_db_is_too_large {
        let new_path = local_root.join(format!("{le}_{year}.db"));
        if !new_path.exists() {
            initialize_database_file(app, &new_path)?;
            return Ok(new_path);
        }
    }

    Ok(latest_path.clone())
}

fn initialize_database_file(
    app: &AppHandle,
    path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut conn = Connection::open(path)?;
    init_tokenizer(app, &conn);
    init_db(&mut conn)?;
    checkpoint(&conn)?;
    Ok(())
}

fn list_le_databases(local_root: &Path, le: &str) -> io::Result<Vec<(PathBuf, i32)>> {
    let prefix = format!("{le}_");
    let mut result = Vec::new();

    for entry in fs::read_dir(local_root)? {
        let path = entry?.path();
        if !path.is_file() || path.extension().and_then(|value| value.to_str()) != Some("db") {
            continue;
        }

        let Some(stem) = path.file_stem().and_then(|value| value.to_str()) else {
            continue;
        };
        let Some(year_text) = stem.strip_prefix(&prefix) else {
            continue;
        };
        let Ok(file_year) = year_text.parse::<i32>() else {
            continue;
        };
        result.push((path, file_year));
    }

    Ok(result)
}

fn backup_on_exit(app: &tauri::AppHandle) {
    let runtime = match app.try_state::<AppRuntimeState>() {
        Some(state) => state,
        None => return,
    };

    if let Some(db_state) = app.try_state::<DbState>() {
        match db_state.conn.lock() {
            Ok(conn) => {
                if let Err(error) = checkpoint(&conn) {
                    eprintln!("退出前 SQLite checkpoint 失败：{error}");
                }
            }
            Err(error) => eprintln!("退出时无法锁定数据库连接：{error}"),
        }
    }

    for le in &runtime.active_le_codes {
        if let Err(error) = backup_le_databases(&runtime.local_root, &runtime.shared_root, le) {
            eprintln!("退出时备份 LE {le} 失败：{error}");
        }
    }
}

fn backup_le_databases(local_root: &Path, shared_root: &Path, le: &str) -> io::Result<()> {
    let databases = list_le_databases(local_root, le)?;
    if databases.is_empty() {
        return Ok(());
    }

    let remote_dir = shared_root.join(le);
    fs::create_dir_all(&remote_dir)?;

    for (source, _) in databases {
        let Some(file_name) = source.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        let destination = remote_dir.join(file_name);
        let temporary = remote_dir.join(format!(".{file_name}.upload-{}", std::process::id()));

        // 先上传临时文件，完成后再替换共享盘中的正式文件。
        // 失败时清理临时文件，避免在共享盘上堆积。rename 会直接覆盖旧文件。
        if let Err(error) = fs::copy(&source, &temporary).and_then(|_| fs::rename(&temporary, &destination)) {
            let _ = fs::remove_file(&temporary);
            return Err(error);
        }
    }

    Ok(())
}

fn checkpoint(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
}

/// 启动时做一次 `integrity_check`。之前发生过 WAL 里索引页写入不完整、
/// checkpoint 时把损坏带入主文件的情况（表数据完好，只有索引 B 树损坏），
/// 与其让用户在业务操作里撞见一句莫名其妙的 "database disk image is
/// malformed"，不如在启动时就明确拦下来、给出可操作的提示。
fn verify_database_integrity(conn: &Connection) -> Result<(), Box<dyn std::error::Error>> {
    let result: String = conn.query_row("PRAGMA integrity_check;", [], |row| row.get(0))?;
    if result != "ok" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "数据库文件已损坏（integrity_check: {result}）。\
                 请勿继续写入，联系管理员用 `sqlite3 <db> \".recover\"` 恢复数据后再启动。"
            ),
        )
        .into());
    }
    Ok(())
}

/// 后台周期性 `wal_checkpoint(PASSIVE)`。
///
/// SQLite 默认的 wal_autocheckpoint 依赖 sqlite3_wal_hook，在有长时间存在的
/// 读连接（例如外部工具打开着同一个文件）时可能一直推进不了，WAL 会无限增长；
/// 一旦某次 checkpoint 因为异常退出被打断，就有可能把不完整的页面带入主文件。
/// 这里不依赖 autocheckpoint，自己定时把 WAL 内容被动合并回主文件，
/// 让单次 checkpoint 处理的量始终很小，降低"整个 WAL 一次性合并时被打断"的概率。
/// PASSIVE 模式不会阻塞并发读写，失败也不影响业务操作，只记录日志。
fn spawn_periodic_checkpoint(conn: Arc<Mutex<Connection>>) {
    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_secs(60));
        match conn.lock() {
            Ok(conn) => {
                if let Err(error) = conn.execute_batch("PRAGMA wal_checkpoint(PASSIVE);") {
                    eprintln!("周期性 WAL checkpoint 失败：{error}");
                }
            }
            Err(error) => eprintln!("周期性 checkpoint 时无法锁定数据库连接：{error}"),
        }
    });
}

fn current_year() -> i32 {
    chrono::Local::now().year()
}
pub fn init_tokenizer(app: &AppHandle, conn: &rusqlite::Connection) {
    // ✅ 开启扩展

    // let lib_path = std::path::PathBuf::from(
    // r"C:\Users\M241452\pyu\mycode\tauri-select-pic\src-tauri\resources\simple.dll"
    // );

    // ✅ 获取 resources 路径
    let lib_path = app
        .path()
        .resource_dir()
        .unwrap()
        .join("resources")
        .join("simple.dll");

    unsafe {
        conn.load_extension_enable().unwrap();
        conn.load_extension(lib_path.to_str().unwrap(), None::<&str>)
            .unwrap();
        // conn.load_extension_disable().unwrap();
    }

    // ✅ 初始化 jieba dict
    let dict_path = app
        .path()
        .resource_dir()
        .unwrap()
        .join("resources")
        .join("dict");
    println!("dict_path: {:?}", dict_path);
    println!("exists: {}", dict_path.exists());
    conn.query_row(
        "SELECT jieba_dict(?1)",
        [&dict_path.to_str().unwrap()],
        |_| Ok(()),
    )
    .unwrap();
}
