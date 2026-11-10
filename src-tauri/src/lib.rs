use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use tauri::Manager;

pub mod backup;
pub mod commands;
pub mod config;
pub mod db;
pub mod error;
pub mod errors;
pub mod excel_reader;
pub mod features;
pub mod init;
pub mod models;
pub mod state;
pub mod sync;
pub mod utils;

use commands::match_invoice::match_reversal_invoices;
use features::ap_voucher_import::import_ap_voucher_excel;
use features::bank_voucher_import::import_bank_voucher_excel;
use features::certified_import::import_certified_excel;
use features::ic_voucher_import::import_ic_voucher_excel;
use features::invoice_import::import_invoice_excel;
use features::travel_expense_import::import_travel_expense_excel;
use commands::startup::{
    get_pending_backups,
    get_startup_status,
    get_startup_progress,
    notify_frontend_ready,
    report_main_progress,
    retry_backup_now,
    restart_app,
    upload_local_to_shared,
};
use features::admin::{
    admin_add_le, admin_add_user, admin_overview, admin_rename_le, admin_set_le_active,
    admin_update_user, current_user_info,
};
use features::input_tax_transfer::{
    confirm_input_tax_transfer, load_input_tax_transfer_review, run_input_tax_transfer_match,
    save_input_tax_transfer_override, save_input_tax_transfer_overrides_batch,
    query_input_tax_transfer_history,
};
use features::layout::{delete_layout, list_layouts, save_layout, set_default_layout};
use features::period_close::{
    get_period_status, lock_tax_period, relock_tax_period, unconfirm_tax_period, unlock_tax_period,
};
use features::query::{
    query_air_details, query_ap_vouchers, query_bank_vouchers, query_building_details,
    query_cargo_details, query_certified_list, query_ic_vouchers, query_invoice_details,
    query_invoice_headers, query_item_overview, query_passenger_details, query_railway_details,
    query_real_estate_details, query_toll_details, query_travel_expense,
};
use commands::le::{list_switchable_les, switch_le};
use commands::storage::{backup_now, get_storage_status};
use features::reversal_query::query_reversal_overview;
use features::export::{
    export_input_tax_transfer, export_input_tax_transfer_accounting, export_input_tax_transfer_history,
    export_item_overview, export_reversal, export_stamp_tax_accounting,
};
use features::rules::{
    add_tax_match_rule, disable_tax_match_rule, export_tax_rules, import_tax_rules, list_stamp_tax_categories,
    list_tax_match_rules,
};
use features::stamp_tax::{
    confirm_stamp_tax, export_stamp_tax_collection_form, load_stamp_tax_review,
    run_stamp_tax_match, save_stamp_tax_override, save_stamp_tax_overrides_batch,
    query_stamp_tax_history, stamp_tax_collection_form_filename,
};
use features::voucher_edit::{
    delete_ap_vouchers, delete_bank_vouchers, delete_ic_vouchers, update_ap_voucher,
    update_bank_voucher, update_ic_voucher,
};

use specta_typescript::Typescript;
use tauri_specta::{collect_commands, collect_events, Builder};

pub use state::{
    AppRuntimeState,
    DbState,
    StartupProgress,
    StartupState,
    StartupStatus,
};

/// 命令与事件清单。run() 和 bin/export_bindings 共用。
fn specta_builder() -> Builder {
    <Builder>::new()
        .commands(collect_commands![
            get_startup_status,
            get_pending_backups,
            retry_backup_now,
            notify_frontend_ready,
            get_startup_progress,
            report_main_progress,
                            upload_local_to_shared,
            restart_app,
            import_certified_excel,
            import_invoice_excel,
            import_travel_expense_excel,
            import_bank_voucher_excel,
            import_ap_voucher_excel,
            import_ic_voucher_excel,
            match_reversal_invoices,
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
            query_reversal_overview,
            export_item_overview,
            export_reversal,
            export_input_tax_transfer,
            export_input_tax_transfer_history,
            export_input_tax_transfer_accounting,
            export_stamp_tax_accounting,
            get_storage_status,
            list_switchable_les,
            switch_le,
            backup_now,
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
            export_tax_rules,
            run_input_tax_transfer_match,
            load_input_tax_transfer_review,
            save_input_tax_transfer_override,
            save_input_tax_transfer_overrides_batch,
            query_input_tax_transfer_history,
            run_stamp_tax_match,
            get_period_status,
            unconfirm_tax_period,
            lock_tax_period,
            unlock_tax_period,
            relock_tax_period,
            confirm_input_tax_transfer,
            confirm_stamp_tax,
            export_stamp_tax_collection_form,
            stamp_tax_collection_form_filename,
            load_stamp_tax_review,
            save_stamp_tax_override,
            save_stamp_tax_overrides_batch,
            query_stamp_tax_history,
        ])
        .events(collect_events![])
}

/// 只导出 TypeScript 绑定到 src/bindings.ts，不启动窗口。
/// 改了命令签名后运行 `cargo run --bin export_bindings` 重新生成。
pub fn export_bindings() {
    specta_builder()
        .export(
            Typescript::default().header("// @ts-nocheck\n"),
            concat!(env!("CARGO_MANIFEST_DIR"), "/../src/bindings.ts"),
        )
        .expect("导出 bindings.ts 失败");
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = specta_builder();

    #[cfg(debug_assertions)]
    builder
        .export(
            Typescript::default().header("// @ts-nocheck\n"),
            "../src/bindings.ts",
        )
        .expect("Failed to export typescript bindings");

    let backup_started = Arc::new(AtomicBool::new(false));
    let backup_started_for_run = Arc::clone(&backup_started);

    let app = tauri::Builder::default()
        // 单实例：同一台机器上只能有一个软件进程在用本地数据库。第二次启动时把已有窗口带到前台，自己退出。
        // 否则两个进程抢同一个 .db，会出现「本地数据库被其他程序占用」。
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            // 优先主窗口；启动失败页（只有 splash 可见）时聚焦 splash
            for label in ["main", "splash"] {
                if let Some(window) = app.get_webview_window(label) {
                    if window.is_visible().unwrap_or(false) {
                        let _ = window.unminimize();
                        let _ = window.set_focus();
                        break;
                    }
                }
            }
        }))
        // 记住主窗口上次的位置 / 大小 / 是否最大化；首次运行用 tauri.conf.json 里的默认值（最大化）。
        // splash 是固定大小的启动画面，不记录，否则它也会被恢复成别的位置。
        // 不能管 VISIBLE：插件默认会在窗口创建时 show()，主窗口会在前端还没渲染时先露出一块空白，
        // 抢在启动页前面。主窗口何时显示由 finish_startup 决定。
        // 也不管 DECORATIONS：本软件固定使用自绘标题栏。
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_denylist(&["splash"])
                .with_state_flags(
                    tauri_plugin_window_state::StateFlags::SIZE
                        | tauri_plugin_window_state::StateFlags::POSITION
                        | tauri_plugin_window_state::StateFlags::MAXIMIZED,
                )
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_process::init())
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            app.manage(StartupState::new());

            let app_handle = app.handle().clone();

            std::thread::spawn(move || {
                match init::initialize(&app_handle) {
                    Ok(initialized) => {
                        let runtime = initialized.runtime.clone();

                        app_handle.manage(DbState::new(initialized.conn));
                        app_handle.manage(state::CurrentLe::new(
                            initialized.runtime.active_le.clone(),
                            initialized.runtime.active_db_path.clone(),
                        ));
                        app_handle.manage(initialized.runtime);

                        backup::spawn_retry_worker(runtime.clone());
                        spawn_periodic_checkpoint(app_handle.state::<DbState>().conn.clone());

                        let state = app_handle.state::<StartupState>();
                        state.set(StartupStatus::Ready {
                            runtime,
                            warnings: initialized.warnings,
                        });
                        // 初始化完成；主窗口是否已渲染决定谁来触发显示。
                        eprintln!("[startup] init ok @ {}", chrono::Local::now().format("%H:%M:%S%.3f"));
                        let ready_now = state.arrive(state::GATE_INIT_OK);
                        commands::startup::flush_main_progress(&app_handle);
                        if ready_now {
                            finish_startup(&app_handle);
                        }
                    }

                    Err(error) => {
                        eprintln!("启动初始化失败：{error}");

                        // 失败：main 保持隐藏，splash 窗口读取 Failed 状态显示错误页。
                        app_handle
                            .state::<StartupState>()
                            .set(StartupStatus::Failed { error });
                    }
                }
            });

            // splash 窗口显示进度与错误；main 在后台加载，初始化成功且前端渲染完才显示。
            //
            // 但窗口"何时显示"和"初始化是否完成"是两件事：这里之前是在
            // setup() 里同步调用 finish_startup，几乎是应用一启动就 show()，
            // 早于 webview 加载完 index.html / JS 并完成首次渲染，用户先看到
            // 一片白屏，隔了好一会儿骨架屏才"跳出来"。
            // WebviewWindow 一旦按 tauri.conf.json 声明式创建，就没有
            // on_page_load 这类钩子可挂了（那是 WebviewWindowBuilder 才有的
            // 方法）。所以改为让前端在真正完成首次渲染（骨架屏已经画出来）后，
            // 主动调用 notify_frontend_ready 命令来触发 show()，而不是
            // Rust 这边自己猜时机。
            builder.mount_events(app);

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("构建 Tauri 应用时发生错误");

    app.run(move |app_handle, event| {
        if matches!(
            event,
            tauri::RunEvent::ExitRequested { .. }
        ) && !backup_started_for_run.swap(true, Ordering::SeqCst)
        {
            backup::backup_on_exit(app_handle);
        }
    });
}

/// 后台周期性 `wal_checkpoint(PASSIVE)`：让 WAL 不会无限增长，也避免退出时一次性合并过大。
/// PASSIVE 不阻塞读写，失败只记录日志。
fn spawn_periodic_checkpoint(conn: Arc<std::sync::Mutex<rusqlite::Connection>>) {
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

/// 初始化成功且主窗口已渲染后调用：关 splash、显示主窗口。
pub(crate) fn finish_startup(app: &tauri::AppHandle) {
    eprintln!("[startup] finish_startup");
    if let Some(splashscreen) = app.get_webview_window("splash") {
        if let Err(error) = splashscreen.close() {
            eprintln!("关闭 splashscreen 失败：{error}");
        }
    }

    match app.get_webview_window("main") {
        Some(main_window) => {
            if let Err(error) = main_window.show() {
                eprintln!("显示 main 窗口失败：{error}");
            }

            if let Err(error) = main_window.set_focus() {
                eprintln!("聚焦 main 窗口失败：{error}");
            }
        }
        None => {
            eprintln!("找不到 label 为 main 的窗口");
        }
    }
}